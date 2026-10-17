//! Runtime shortcuts for Hyprland's Lua configuration (0.55 and newer).

use std::{
    io::{self, BufRead, BufReader},
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use global_hotkey::{
    GlobalHotKeyEvent, HotKeyState,
    hotkey::{HotKey, Modifiers},
};
use serde::Deserialize;
use thiserror::Error;

use super::{ListenerMessage, keysym::keysym};
use crate::linux::hyprland::Connection;

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
enum Error {
    #[error("invalid or unsupported Hyprland shortcut")]
    Unsupported,
    #[error("shortcut is already bound in Hyprland")]
    Conflict,
    #[error("Hyprland IPC: {0}")]
    Io(#[from] io::Error),
    #[error("invalid Hyprland bindings response: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Ipc(#[from] crate::linux::hyprland::Error),
}

struct Chord {
    lua_keys: String,
    key: String,
    modmask: u32,
    id: u32,
}

impl Chord {
    fn parse(input: &str) -> Result<Self, Error> {
        let hotkey = input.parse::<HotKey>().map_err(|_| Error::Unsupported)?;
        let code = hotkey.key.to_string();
        let key = keysym(&code).ok_or(Error::Unsupported)?;
        let mut parts = Vec::new();
        let mut modmask = 0;
        for (modifier, name, mask) in [
            (Modifiers::CONTROL, "CTRL", 4),
            (Modifiers::SHIFT, "SHIFT", 1),
            (Modifiers::ALT, "ALT", 8),
            (Modifiers::SUPER, "SUPER", 64),
        ] {
            if hotkey.mods.contains(modifier) {
                parts.push(name);
                modmask |= mask;
            }
        }
        parts.push(&key);
        Ok(Self {
            lua_keys: parts.join(" + "),
            key,
            modmask,
            id: hotkey.id,
        })
    }

    fn check_available(&self, json: &str) -> Result<(), Error> {
        self.check_available_except(json, "")
    }

    fn check_available_except(&self, json: &str, own_arg: &str) -> Result<(), Error> {
        let binds: Vec<Binding> = serde_json::from_str(json)?;
        // j/binds does not include the keymap needed to resolve numeric keycodes.
        // Reject confirmed symbol/catch-all conflicts; an unknown code alone is
        // not evidence that this chord is occupied.
        if binds.iter().any(|bind| {
            !(bind.handler == "__lua" && !own_arg.is_empty() && bind.arg == own_arg)
                && bind.submap.is_empty()
                && (bind.modmask == self.modmask || bind.ignore_mods)
                && (bind.key.eq_ignore_ascii_case(&self.key) || bind.catch_all)
        }) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Binding {
    modmask: u32,
    key: String,
    submap: String,
    #[serde(default, rename = "dispatcher")]
    handler: String,
    #[serde(default)]
    arg: String,
    #[serde(default)]
    catch_all: bool,
    #[serde(default)]
    ignore_mods: bool,
}

fn install(connection: &Connection, chord: &Chord, token: &str) -> Result<(), Error> {
    let slot = format!("_ropy_hotkeys[{}]", chord.id);
    let own_arg = connection.request(&format!(
        "/repl (_ropy_hotkeys and {slot} and {slot}.bind.arg) or ''"
    ))?;
    let binds = connection.request("j/binds")?;
    if own_arg.trim().is_empty() {
        chord.check_available(&binds)?;
    } else {
        chord.check_available_except(&binds, own_arg.trim())?;
    }
    // Only generated ASCII keysyms and numeric IDs enter this Lua program.
    // Keep the handle: unbind/remove also removes OTHER bindings on the same chord.
    connection.eval(&format!(
            "assert(hl.get_current_submap() == '', 'active submap'); _ropy_hotkeys = _ropy_hotkeys or {{}}; \
             local e = {slot}; if e and e.bind:is_enabled() ~= nil then \
             e.token = '{token}'; e.bind:set_enabled(true); else \
             e = {{token = '{token}'}}; {slot} = e; \
             e.bind = hl.bind('{}', function() hl.dispatch(hl.dsp.event(e.token)) end); end",
            chord.lua_keys
        ))?;
    Ok(())
}

fn disable(connection: &Connection, id: u32, token: &str) {
    if let Err(error) = connection.eval(&format!(
        "local e = _ropy_hotkeys and _ropy_hotkeys[{id}]; \
             if e and e.token == '{token}' then e.bind:set_enabled(false) end"
    )) {
        tracing::warn!(%error, "failed to disable Hyprland shortcut");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Activate,
    Reload,
}

fn event(line: &str, token: &str) -> Option<Event> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.strip_prefix("custom>>") == Some(token) {
        Some(Event::Activate)
    } else if line == "configreloaded>>" {
        Some(Event::Reload)
    } else {
        None
    }
}

pub(super) struct Registration {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Registration {
    pub(super) fn register(
        input: &str,
        sender: async_channel::Sender<ListenerMessage>,
    ) -> Option<Self> {
        Self::try_register(input, sender)
            .map_err(|error| {
                tracing::warn!(%error, "failed to register Hyprland shortcut");
            })
            .ok()
    }

    fn try_register(
        input: &str,
        sender: async_channel::Sender<ListenerMessage>,
    ) -> Result<Self, Error> {
        let chord = Chord::parse(input)?;
        let connection = Connection::current().ok_or(Error::Unsupported)?;
        // Subscribe before installing, so activation/reload cannot be lost between them.
        let socket = connection.events()?;
        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        let token = format!(
            "ropy:{}:{}",
            std::process::id(),
            NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
        );
        install(&connection, &chord, &token)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let cleanup = connection.clone();
        let cleanup_token = token.clone();
        let id = chord.id;
        let worker = thread::Builder::new()
            .name("hyprland-hotkey".into())
            .spawn(move || {
                listen(socket, &connection, &chord, &token, &sender, &worker_stop);
                disable(&connection, chord.id, &token);
            });
        match worker {
            Ok(worker) => Ok(Self {
                stop,
                worker: Some(worker),
            }),
            Err(error) => {
                disable(&cleanup, id, &cleanup_token);
                Err(error.into())
            }
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn listen(
    socket: UnixStream,
    connection: &Connection,
    chord: &Chord,
    token: &str,
    sender: &async_channel::Sender<ListenerMessage>,
    stop: &AtomicBool,
) {
    let mut reader = BufReader::new(socket);
    let mut line = String::new();
    while !stop.load(Ordering::Acquire) {
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                match event(&line, token) {
                    Some(Event::Activate) => {
                        tracing::debug!(id = chord.id, "Hyprland shortcut activated");
                        if sender
                            .try_send(ListenerMessage::HotkeyEvent(GlobalHotKeyEvent {
                                id: chord.id,
                                state: HotKeyState::Pressed,
                            }))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Some(Event::Reload) => {
                        tracing::debug!(id = chord.id, "restoring Hyprland shortcut after reload");
                        if let Err(error) = install(connection, chord, token) {
                            tracing::warn!(%error, "failed to restore Hyprland shortcut after reload");
                        }
                    }
                    None => {}
                }
                line.clear();
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => {
                tracing::warn!(%error, "Hyprland shortcut event stream disconnected");
                break;
            }
        }
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn send_test_shortcut() {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        let mut input = Enigo::new(&Settings {
            x11_display: Some("disabled".into()),
            ..Settings::default()
        })
        .expect("Wayland virtual keyboard");
        input.key(Key::Control, Direction::Press).expect("control");
        input.key(Key::Shift, Direction::Press).expect("shift");
        input.key(Key::F12, Direction::Click).expect("F12");
        input
            .key(Key::Shift, Direction::Release)
            .expect("release shift");
        input
            .key(Key::Control, Direction::Release)
            .expect("release control");
    }

    #[test]
    #[ignore = "requires a live Hyprland Lua session; temporarily binds Ctrl+Shift+F12 and reloads config"]
    fn test_live_hyprland_activation_reload_and_cleanup() {
        let connection = Connection::current().expect("Hyprland session");
        let chord = Chord::parse("ctrl+shift+f12").expect("chord");
        let (sender, receiver) = async_channel::unbounded();
        let registration = Registration::try_register("ctrl+shift+f12", sender).expect("register");
        let token_request = format!("/repl _ropy_hotkeys[{}].token", chord.id);
        let token = connection.request(&token_request).expect("token");
        send_test_shortcut();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while receiver.is_empty() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(
            receiver.try_recv(),
            Ok(ListenerMessage::HotkeyEvent(_))
        ));
        connection.request("/reload config-only").expect("reload");
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if connection
                .request(&token_request)
                .is_ok_and(|value| value.trim() == token.trim())
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "binding was not restored"
            );
            thread::sleep(Duration::from_millis(20));
        }
        send_test_shortcut();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while receiver.is_empty() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(
            receiver.try_recv(),
            Ok(ListenerMessage::HotkeyEvent(_))
        ));
        drop(registration);
        let enabled = connection
            .request(&format!(
                "/repl _ropy_hotkeys[{}].bind:is_enabled()",
                chord.id
            ))
            .expect("enabled");
        assert_eq!(enabled.trim(), "false");
        let (sender, _) = async_channel::unbounded();
        let registration = Registration::try_register("ctrl+shift+f12", sender)
            .expect("reuse disabled owned binding");
        drop(registration);
    }

    #[rstest]
    #[case("ctrl+shift+d", "CTRL + SHIFT + D", 5)]
    #[case("super+alt+space", "ALT + SUPER + space", 72)]
    #[case("ctrl+enter", "CTRL + Return", 4)]
    #[case("ctrl+1", "CTRL + 1", 4)]
    #[case("alt+f12", "ALT + F12", 8)]
    fn test_chord_supported_keys_translate(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] mask: u32,
    ) {
        let chord = Chord::parse(input).expect("valid chord");
        assert_eq!(chord.lua_keys, expected);
        assert_eq!(chord.modmask, mask);
    }

    #[test]
    fn test_conflict_case_insensitive_key_rejected() {
        let chord = Chord::parse("ctrl+shift+d").expect("chord");
        let binds = r#"[{"modmask":5,"key":"d","keycode":0,"submap":""}]"#;
        assert!(chord.check_available(binds).is_err());
        assert!(chord.check_available("[]").is_ok());
        assert!(chord.check_available("invalid json").is_err());
    }

    #[rstest]
    #[case(r#"{"modmask":5,"key":"F1","keycode":67,"submap":""}"#, false)]
    #[case(r#"{"modmask":5,"key":"","keycode":67,"submap":""}"#, false)]
    #[case(r#"{"modmask":5,"key":"d","keycode":0,"submap":""}"#, true)]
    #[case(r#"{"modmask":4,"key":"d","keycode":0,"submap":""}"#, false)]
    #[case(r#"{"modmask":5,"key":"d","keycode":0,"submap":"other"}"#, false)]
    #[case(
        r#"{"modmask":5,"key":"","keycode":0,"submap":"","catch_all":true}"#,
        true
    )]
    #[case(
        r#"{"modmask":0,"key":"d","keycode":0,"submap":"","ignore_mods":true}"#,
        true
    )]
    fn test_conflict_requires_matching_chord(#[case] binding: &str, #[case] conflict: bool) {
        let chord = Chord::parse("ctrl+shift+d").expect("chord");
        assert_eq!(
            matches!(
                chord.check_available(&format!("[{binding}]")),
                Err(Error::Conflict)
            ),
            conflict,
        );
    }

    #[test]
    fn test_conflict_owned_handle_does_not_hide_other_binding() {
        let chord = Chord::parse("ctrl+shift+d").expect("chord");
        let owned =
            r#"[{"modmask":5,"key":"D","keycode":0,"submap":"","dispatcher":"__lua","arg":"12"}]"#;
        assert!(chord.check_available_except(owned, "12").is_ok());
        let other =
            r#"[{"modmask":5,"key":"D","keycode":0,"submap":"","dispatcher":"__lua","arg":"13"}]"#;
        assert!(chord.check_available_except(other, "12").is_err());
    }

    #[test]
    fn test_event_only_exact_activation_token_matches() {
        assert_eq!(
            event("custom>>ropy:123:456\n", "ropy:123:456"),
            Some(Event::Activate)
        );
        assert_eq!(event("custom>>ropy:123:4567\n", "ropy:123:456"), None);
        assert_eq!(
            event("configreloaded>>\n", "ropy:123:456"),
            Some(Event::Reload)
        );
        assert_eq!(event("activewindow>>anything\n", "ropy:123:456"), None);
    }
}
