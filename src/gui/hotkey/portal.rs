//! Desktop portal global shortcuts for native Wayland sessions.

use ashpd::desktop::{
    CreateSessionOptions,
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
};
use ashpd::zbus;
use futures_lite::{StreamExt, future};
use global_hotkey::hotkey::{HotKey, Modifiers};
use gpui_kit::{BackgroundExecutor, Task};
use thiserror::Error;

use super::ListenerMessage;

const SHORTCUT_ID: &str = "show";

#[derive(Debug, Error)]
enum Error {
    #[error("invalid or unsupported portal shortcut")]
    Unsupported,
    #[error("shortcut registration cancelled")]
    Cancelled,
    #[error("portal did not bind the requested shortcut")]
    MissingShortcut,
    #[error(transparent)]
    Portal(#[from] ashpd::Error),
    #[error(transparent)]
    Bus(#[from] zbus::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn preferred_trigger(input: &str) -> Option<String> {
    let hotkey = input.parse::<HotKey>().ok()?;
    let key = super::keysym::keysym(&hotkey.key.to_string())?;
    let mut parts = Vec::new();
    for (modifier, name) in [
        (Modifiers::CONTROL, "CTRL"),
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::META, "LOGO"),
    ] {
        if hotkey.mods.contains(modifier) {
            parts.push(name);
        }
    }
    parts.push(&key);
    Some(parts.join("+"))
}

pub(super) struct Registration {
    stop: async_channel::Sender<()>,
    worker: Option<Task<()>>,
}

impl Registration {
    pub(super) async fn register(
        hotkey: String,
        description: String,
        sender: async_channel::Sender<ListenerMessage>,
        cancelled: async_channel::Receiver<()>,
        completion: async_channel::Sender<()>,
        executor: BackgroundExecutor,
    ) -> Option<Self> {
        let result =
            Self::connect(hotkey, description, sender, cancelled, completion, executor).await;
        result
            .map_err(|error| tracing::warn!(%error, "failed to register portal shortcut"))
            .ok()
    }

    async fn connect(
        hotkey: String,
        description: String,
        sender: async_channel::Sender<ListenerMessage>,
        cancelled: async_channel::Receiver<()>,
        completion: async_channel::Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<Self, Error> {
        let trigger = preferred_trigger(&hotkey).ok_or(Error::Unsupported)?;
        let id = hotkey.parse::<HotKey>().map_err(|_| Error::Unsupported)?.id;
        // A private connection lets cancellation close pending portal requests
        // and sessions without disturbing clipboard or other GPUI portal users.
        let connection = future::race(
            async { zbus::Connection::session().await.map_err(Error::from) },
            async {
                let _ = cancelled.recv().await;
                Err(Error::Cancelled)
            },
        )
        .await?;
        let setup = async {
            let portal = GlobalShortcuts::with_connection(connection.clone()).await?;
            let session = portal
                .create_session(CreateSessionOptions::default())
                .await?;
            let mut events = portal.receive_activated().await?;
            let shortcuts =
                [NewShortcut::new(SHORTCUT_ID, description).preferred_trigger(trigger.as_str())];
            let response = portal
                .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
                .await?
                .response()?;
            if !response
                .shortcuts()
                .iter()
                .any(|shortcut| shortcut.id() == SHORTCUT_ID)
            {
                return Err(Error::MissingShortcut);
            }
            let session_path: String = serde_json::from_value(serde_json::to_value(&session)?)?;
            let (stop, stopped) = async_channel::bounded::<()>(1);
            let worker_connection = connection.clone();
            let worker_completion = completion.clone();
            let worker_cancelled = cancelled.clone();
            let worker = executor.spawn(async move {
                loop {
                    let event = future::race(events.next(), async {
                        let _ = future::race(stopped.recv(), worker_cancelled.recv()).await;
                        None
                    })
                    .await;
                    let Some(event) = event else { break };
                    if event.session_handle().as_str() != session_path
                        || event.shortcut_id() != SHORTCUT_ID
                    {
                        continue;
                    }
                    let token = event
                        .options()
                        .get("activation_token")
                        .and_then(|value| value.downcast_ref::<&str>().ok())
                        .map(str::to_owned);
                    if sender
                        .try_send(ListenerMessage::PortalActivation { id, token })
                        .is_err()
                    {
                        break;
                    }
                }
                let _ = session.close().await;
                let _ = worker_connection.close().await;
                drop(worker_completion);
            });
            Ok(Self {
                stop,
                worker: Some(worker),
            })
        };
        let result = future::race(setup, async {
            let _ = cancelled.recv().await;
            Err(Error::Cancelled)
        })
        .await;
        if result.is_err() {
            let _ = connection.close().await;
        }
        result
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.stop.close();
        if let Some(worker) = self.worker.take() {
            worker.detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portal_trigger_uses_xdg_modifier_names_and_keysyms() {
        assert_eq!(
            preferred_trigger("ctrl+shift+d").as_deref(),
            Some("CTRL+SHIFT+D")
        );
        assert_eq!(
            preferred_trigger("super+alt+enter").as_deref(),
            Some("ALT+LOGO+Return")
        );
        assert_eq!(
            preferred_trigger("ctrl+ArrowLeft").as_deref(),
            Some("CTRL+Left")
        );
        assert_eq!(preferred_trigger("not+a+shortcut"), None);
    }
}
