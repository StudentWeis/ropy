use std::{error::Error, io};

use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

/// Native X11 window operations for the Ropy application window.
pub struct X11 {
    connection: RustConnection,
    root_id: u32,
    net_wm_state_above: u32,
    net_wm_state: u32,
    window_id: u32,
    net_active_window: u32,
}

impl std::fmt::Debug for X11 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X11")
            .field("root_id", &self.root_id)
            .field("window_id", &self.window_id)
            .finish_non_exhaustive()
    }
}

impl X11 {
    /// Creates a new X11 instance by connecting to the X server and finding
    /// the current process's main application window.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot connect to the X server.
    /// - Cannot find a window with the expected application ID belonging to
    ///   the current process.
    pub fn new(expected_app_id: &str) -> Result<Self, Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;

        let screen = &conn.setup().roots[screen_num];
        let root_id = screen.root;
        let net_client_list = Self::get_atom(&conn, b"_NET_CLIENT_LIST")?;
        let net_wm_pid = Self::get_atom(&conn, b"_NET_WM_PID")?;
        let net_wm_state_above = Self::get_atom(&conn, b"_NET_WM_STATE_ABOVE")?;
        let net_wm_state = Self::get_atom(&conn, b"_NET_WM_STATE")?;
        let net_active_window = Self::get_atom(&conn, b"_NET_ACTIVE_WINDOW")?;

        let windows = Self::get_value32(&conn, root_id, net_client_list)?;

        let process_id = std::process::id();
        let mut window_id = None;

        for window in windows {
            let Ok(pids) = Self::get_value32(&conn, window, net_wm_pid) else {
                continue;
            };
            let Ok(wm_class) = Self::get_value8(&conn, window, AtomEnum::WM_CLASS.into()) else {
                continue;
            };

            if window_identity_matches(&pids, &wm_class, process_id, expected_app_id) {
                window_id = Some(window);
                break;
            }
        }

        Ok(Self {
            connection: conn,
            root_id,
            net_wm_state_above,
            net_wm_state,
            window_id: window_id.ok_or_else(|| io::Error::other("Failed to get window id"))?,
            net_active_window,
        })
    }

    fn get_atom(conn: &RustConnection, cmd: &[u8]) -> Result<u32, Box<dyn Error>> {
        let atom = conn.intern_atom(false, cmd)?;
        let atom = atom.reply()?.atom;

        Ok(atom)
    }

    fn get_value32(
        conn: &RustConnection,
        window: u32,
        atom: u32,
    ) -> Result<Vec<u32>, Box<dyn Error>> {
        let reply = conn
            .get_property(false, window, atom, AtomEnum::ANY, 0, u32::MAX)?
            .reply()?;

        let res = reply
            .value32()
            .ok_or_else(|| io::Error::other("Failed to get reply"))?
            .collect();

        Ok(res)
    }

    fn get_value8(
        conn: &RustConnection,
        window: u32,
        atom: u32,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(conn
            .get_property(false, window, atom, AtomEnum::ANY, 0, u32::MAX)?
            .reply()?
            .value)
    }

    fn send_wm_state_and_sync(
        &self,
        status: u32,
        enable: bool,
        window: u32,
    ) -> Result<(), Box<dyn Error + 'static>> {
        let event = ClientMessageEvent::new(
            32,
            self.window_id,
            self.net_wm_state,
            [u32::from(enable), status, 0, 0, 0],
        );

        self.connection.send_event(
            false,
            window,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;

        self.connection.sync()?;

        Ok(())
    }

    /// Sets the always-on-top state for the window.
    ///
    /// # Errors
    ///
    /// Returns an error if the X11 connection fails or the window state cannot be updated.
    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<(), Box<dyn Error>> {
        self.send_wm_state_and_sync(self.net_wm_state_above, always_on_top, self.root_id)
    }

    /// Displays the window and activates it (brings to foreground).
    ///
    /// # Errors
    ///
    /// Returns an error if the X11 request cannot be sent. Window managers
    /// are allowed to decline activation requests without reporting an error.
    pub fn display_and_activate_window(&self) -> Result<(), Box<dyn Error>> {
        self.display_window()?;
        self.active_window()?;

        Ok(())
    }

    /// Displays (maps) the window without activating it.
    ///
    /// # Errors
    ///
    /// Returns an error if the X11 connection fails or the window cannot be mapped.
    pub fn display_window(&self) -> Result<(), Box<dyn Error>> {
        self.connection.map_window(self.window_id)?;
        self.connection.sync()?;

        Ok(())
    }

    /// Activates the window (brings to foreground) by sending a `_NET_ACTIVE_WINDOW` client message.
    ///
    /// # Errors
    ///
    /// Returns an error if the X11 request cannot be sent. The request is
    /// asynchronous: the window manager may decline it without reporting an
    /// error to the client.
    pub fn active_window(&self) -> Result<(), Box<dyn Error>> {
        let event = ClientMessageEvent::new(
            32,
            self.window_id,
            self.net_active_window,
            [2, x11rb::CURRENT_TIME, 0, 0, 0],
        );

        self.connection.send_event(
            false,
            self.root_id,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;

        self.connection.sync()?;

        Ok(())
    }

    /// Hides (unmaps) the window.
    ///
    /// # Errors
    ///
    /// Returns an error if the X11 connection fails or the window cannot be unmapped.
    pub fn hide_window(&self) -> Result<(), Box<dyn Error>> {
        self.connection.unmap_window(self.window_id)?;
        self.connection.sync()?;

        Ok(())
    }
}

fn window_identity_matches(
    pids: &[u32],
    wm_class: &[u8],
    expected_pid: u32,
    expected_app_id: &str,
) -> bool {
    pids.contains(&expected_pid)
        && wm_class
            .split(|byte| *byte == 0)
            .any(|class| class == expected_app_id.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::window_identity_matches;

    #[test]
    fn selects_main_window_when_same_process_owns_helper_windows() {
        let process_id = 42;

        assert!(!window_identity_matches(
            &[process_id],
            b"tray-helper\0TrayHelper\0",
            process_id,
            "Ropy",
        ));
        assert!(window_identity_matches(
            &[process_id],
            b"Ropy\0Ropy\0",
            process_id,
            "Ropy",
        ));
    }

    #[test]
    fn rejects_matching_window_class_from_another_process() {
        assert!(!window_identity_matches(&[7], b"Ropy\0Ropy\0", 42, "Ropy",));
    }
}
