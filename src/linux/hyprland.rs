//! Hyprland session detection, IPC transport and popup activation.

use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    thread,
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;

const TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Hyprland IPC: {0}")]
    Io(#[from] io::Error),
    #[error("invalid Hyprland response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Hyprland rejected request: {0}")]
    Rejected(String),
}

#[derive(Clone)]
pub(crate) struct Connection {
    directory: PathBuf,
}

impl Connection {
    pub(crate) fn current() -> Option<Self> {
        let signature = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
        if signature.is_empty() {
            return None;
        }
        Some(Self {
            directory: PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
                .join("hypr")
                .join(signature),
        })
    }

    pub(crate) fn events(&self) -> io::Result<UnixStream> {
        UnixStream::connect(self.directory.join(".socket2.sock"))
    }

    pub(crate) fn request(&self, request: &str) -> Result<String, Error> {
        let mut socket = UnixStream::connect(self.directory.join(".socket.sock"))?;
        socket.set_read_timeout(Some(TIMEOUT))?;
        socket.set_write_timeout(Some(TIMEOUT))?;
        socket.write_all(request.as_bytes())?;
        let mut response = String::new();
        socket.take(1024 * 1024).read_to_string(&mut response)?;
        Ok(response)
    }

    pub(crate) fn eval(&self, lua: &str) -> Result<(), Error> {
        let response = self.request(&format!("/eval {lua}"))?;
        if response.trim() == "ok" {
            Ok(())
        } else {
            Err(Error::Rejected(response))
        }
    }
}

/// Own the native popup's runtime rule for the application lifetime. Reuse the
/// Lua handle across reconstructions; a configuration reload invalidates it.
pub(crate) struct PopupRules(Connection);

impl PopupRules {
    pub(crate) fn current() -> Option<Self> {
        is_session().then(Connection::current).flatten().map(Self)
    }

    pub(crate) fn prepare(&self, width: f32, height: f32) -> Result<(), Error> {
        // Install before GPUI creates the surface: floating after map would
        // briefly rearrange the user's tiled windows. Match only our native
        // app ID, leaving XWayland and other applications alone.
        self.0.eval(&format!(
            "if _ropy_popup_rule and _ropy_popup_rule:is_enabled() ~= nil then \
             _ropy_popup_rule:set_enabled(true); else \
             _ropy_popup_rule = hl.window_rule({{ \
             match = {{class = '^com[.]ropy[.]app$', xwayland = false}}, \
             float = true, center = true, size = {{{width}, {height}}} }}); end"
        ))
    }
}

impl Drop for PopupRules {
    fn drop(&mut self) {
        if let Err(error) = self.0.eval(
            "if _ropy_popup_rule and _ropy_popup_rule:is_enabled() ~= nil then \
             _ropy_popup_rule:set_enabled(false); end",
        ) {
            tracing::warn!(%error, "failed to disable Hyprland popup rule");
        }
    }
}

/// Map requests are asynchronous. Wait briefly for our mapped window, then
/// focus only this process in response to an explicit user activation. This
/// also restores focus to a pinned window activated through the CLI or tray.
pub(crate) fn focus_popup() {
    let Some(connection) = Connection::current() else {
        return;
    };
    let pid = std::process::id();
    for _ in 0..10 {
        match connection
            .request("j/clients")
            .and_then(|json| has_mapped_popup(&json, pid))
        {
            Ok(true) => {
                if let Err(error) = connection.eval(&format!(
                    "hl.dispatch(hl.dsp.focus({{window = 'pid:{pid}'}}))"
                )) {
                    tracing::warn!(%error, "failed to focus Hyprland popup");
                }
                return;
            }
            Ok(false) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                tracing::warn!(%error, "failed to locate Hyprland popup");
                return;
            }
        }
    }
}

fn has_mapped_popup(json: &str, pid: u32) -> Result<bool, Error> {
    #[derive(Deserialize)]
    struct Client {
        pid: i64,
        mapped: bool,
    }
    let clients: Vec<Client> = serde_json::from_str(json)?;
    Ok(clients
        .iter()
        .any(|client| client.pid == i64::from(pid) && client.mapped))
}

pub(crate) fn is_session() -> bool {
    crate::linux::is_wayland()
        && std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an isolated Hyprland Lua session; installs popup rules and reloads config"]
    fn test_live_hyprland_popup_rule_reuse_reload_and_cleanup() {
        let connection = Connection::current().expect("Hyprland session");
        let rules = PopupRules(connection.clone());
        rules.prepare(400.0, 550.0).expect("install rule");
        connection
            .eval("assert(_ropy_popup_rule:is_enabled()); _ropy_previous_rule = _ropy_popup_rule")
            .expect("enabled rule");
        rules.prepare(400.0, 550.0).expect("reuse rule");
        connection
            .eval("assert(_ropy_previous_rule == _ropy_popup_rule)")
            .expect("no duplicate rule");
        drop(rules);
        connection
            .eval("assert(_ropy_popup_rule:is_enabled() == false)")
            .expect("disabled on exit");
        let rules = PopupRules(connection.clone());
        rules.prepare(400.0, 550.0).expect("restart");
        connection
            .eval(
                "assert(_ropy_previous_rule == _ropy_popup_rule and _ropy_popup_rule:is_enabled())",
            )
            .expect("restart reuses disabled rule");
        connection.request("/reload config-only").expect("reload");
        rules.prepare(400.0, 550.0).expect("restore after reload");
        connection
            .eval("assert(_ropy_popup_rule:is_enabled())")
            .expect("restored rule");
    }

    #[test]
    fn test_focus_only_targets_mapped_window_of_current_process() {
        let clients = r#"[{"pid":10,"mapped":false},{"pid":20,"mapped":true}]"#;
        assert!(!has_mapped_popup(clients, 10).expect("clients"));
        assert!(has_mapped_popup(clients, 20).expect("clients"));
        assert!(!has_mapped_popup(clients, 30).expect("clients"));
        assert!(has_mapped_popup("bad response", 20).is_err());
    }
}
