use std::error::Error;

use x11rb::{
    connection::Connection,
    protocol::xproto::{
        AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, EventMask, PropMode,
    },
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
    /// Connects to the X server for an existing GPUI window, including an
    /// initially hidden window that is not in the window manager's client list.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or atom lookup fails.
    pub fn new(window_id: u32) -> Result<Self, Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;

        let screen = &conn.setup().roots[screen_num];
        let root_id = screen.root;
        let net_wm_state_above = Self::get_atom(&conn, b"_NET_WM_STATE_ABOVE")?;
        let net_wm_state = Self::get_atom(&conn, b"_NET_WM_STATE")?;
        let net_active_window = Self::get_atom(&conn, b"_NET_ACTIVE_WINDOW")?;

        // GPUI's PopUp bypasses the WM. Native move requests require a
        // managed window; utility preserves the floating tool-window role.
        conn.change_window_attributes(
            window_id,
            &ChangeWindowAttributesAux::new().override_redirect(0),
        )?
        .check()?;
        let window_type = Self::get_atom(&conn, b"_NET_WM_WINDOW_TYPE")?;
        let utility = Self::get_atom(&conn, b"_NET_WM_WINDOW_TYPE_UTILITY")?;
        conn.change_property32(
            PropMode::REPLACE,
            window_id,
            window_type,
            AtomEnum::ATOM,
            &[utility],
        )?
        .check()?;
        // Ropy draws its own header, including without an X11 compositor.
        let motif_hints = Self::get_atom(&conn, b"_MOTIF_WM_HINTS")?;
        conn.change_property32(
            PropMode::REPLACE,
            window_id,
            motif_hints,
            motif_hints,
            &[2, 0, 0, 0, 0],
        )?
        .check()?;

        Ok(Self {
            connection: conn,
            root_id,
            net_wm_state_above,
            net_wm_state,
            window_id,
            net_active_window,
        })
    }

    fn get_atom(conn: &RustConnection, cmd: &[u8]) -> Result<u32, Box<dyn Error>> {
        let atom = conn.intern_atom(false, cmd)?;
        let atom = atom.reply()?.atom;

        Ok(atom)
    }

    /// Query actual focus: WM keyboard grabs emit `FocusOut` without changing it.
    ///
    /// # Errors
    /// Returns an error if the X server cannot report the input focus.
    pub fn has_input_focus(&self) -> Result<bool, Box<dyn Error>> {
        Ok(self.connection.get_input_focus()?.reply()?.focus == self.window_id)
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
