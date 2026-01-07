use std::{error::Error, io};

use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

#[allow(dead_code)]
pub struct X11 {
    connection: RustConnection,
    root_id: u32,
    net_client_list: u32,
    net_wm_pid: u32,
    net_wm_state_above: u32,
    net_wm_state: u32,
    window_id: u32,
}

impl X11 {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;

        let screen = &conn.setup().roots[screen_num];
        let root_id = screen.root;
        let net_client_list = Self::get_atom(&conn, b"_NET_CLIENT_LIST")?;
        let net_wm_pid = Self::get_atom(&conn, b"_NET_WM_PID")?;
        let net_wm_state_above = Self::get_atom(&conn, b"_NET_WM_STATE_ABOVE")?;
        let net_wm_state = Self::get_atom(&conn, b"_NET_WM_STATE")?;

        let windows = Self::get_value32(&conn, root_id, net_client_list)?;

        let mut window_id = None;

        for window in windows {
            let pids = Self::get_value32(&conn, window, net_wm_pid)?;

            if pids.contains(&std::process::id()) {
                window_id = Some(window);
                break;
            }
        }

        Ok(Self {
            connection: conn,
            root_id,
            net_client_list,
            net_wm_pid,
            net_wm_state_above,
            net_wm_state,
            window_id: window_id.ok_or_else(|| io::Error::other("Failed to get window id"))?,
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
            [if enable { 1 } else { 0 }, status, 0, 0, 0],
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

    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<(), Box<dyn Error>> {
        self.send_wm_state_and_sync(self.net_wm_state_above, always_on_top, self.root_id)
    }
}
