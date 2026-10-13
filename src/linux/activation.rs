//! Apply a compositor-issued activation token to a live GPUI Wayland surface.

use gpui_kit::Window;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use thiserror::Error;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::{Backend, ObjectId},
    protocol::{wl_registry, wl_surface::WlSurface},
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

#[derive(Debug, Error)]
pub(crate) enum ActivationError {
    #[error("native window handle unavailable: {0}")]
    Handle(#[from] raw_window_handle::HandleError),
    #[error("window does not have a Wayland surface")]
    NotWayland,
    #[error("invalid Wayland object: {0}")]
    InvalidObject(#[from] wayland_client::backend::InvalidId),
    #[error("failed to discover Wayland activation: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
    #[error("failed to send Wayland activation: {0}")]
    Flush(#[from] wayland_client::backend::WaylandError),
    #[error("compositor does not support xdg_activation_v1")]
    Unsupported,
}

/// Own the activation proxy while retaining GPUI's platform, which owns the
/// borrowed libwayland display even when the popup has no native window.
pub(crate) struct Activation {
    proxy: XdgActivationV1,
    connection: Connection,
    _platform: std::rc::Rc<dyn gpui_kit::Platform>,
}

impl Activation {
    pub(crate) fn new(
        window: &Window,
        platform: std::rc::Rc<dyn gpui_kit::Platform>,
    ) -> Result<Self, ActivationError> {
        let RawDisplayHandle::Wayland(display) = window.display_handle()?.as_raw() else {
            return Err(ActivationError::NotWayland);
        };
        // SAFETY: the retained platform owns this live libwayland display. The
        // foreign backend neither owns nor disconnects it and is dropped first.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let mut queue = connection.new_event_queue();
        let registry = connection.display().get_registry(&queue.handle(), ());
        let mut state = State::default();
        let result = queue.roundtrip(&mut state);
        connection.backend().destroy_object(&registry.id())?;
        result?;
        let proxy = state.activation.ok_or(ActivationError::Unsupported)?;
        Ok(Self {
            proxy,
            connection,
            _platform: platform,
        })
    }

    pub(crate) fn activate(&self, window: &Window, token: &str) -> Result<(), ActivationError> {
        let RawWindowHandle::Wayland(handle) = HasWindowHandle::window_handle(window)?.as_raw()
        else {
            return Err(ActivationError::NotWayland);
        };
        // SAFETY: the live window owns this surface on the retained platform.
        // The proxy is borrowed only for this request and is never destroyed.
        let id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), handle.surface.as_ptr().cast())? };
        let surface = WlSurface::from_id(&self.connection, id)?;
        self.proxy.activate(token.to_owned(), &surface);
        self.connection.flush()?;
        Ok(())
    }
}

impl Drop for Activation {
    fn drop(&mut self) {
        self.proxy.destroy();
        let _ = self.connection.flush();
    }
}

#[derive(Default)]
struct State {
    activation: Option<XdgActivationV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
            && interface == XdgActivationV1::interface().name
        {
            state.activation = Some(registry.bind(name, 1, queue, ()));
        }
    }
}

wayland_client::delegate_noop!(State: ignore XdgActivationV1);
