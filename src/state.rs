use std::collections::HashMap;

use wayland_client::{
    self, Connection, Dispatch, DispatchError, EventQueue,
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer, wl_compositor::WlCompositor, wl_display::WlDisplay, wl_registry::{self, WlRegistry}, wl_shm::WlShm, wl_shm_pool::WlShmPool, wl_surface::WlSurface
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

use crate::{gpu_surface::GlAbstraction, surface::{Surface, UninitSurface}};

#[derive(Debug, Clone, Default)]
pub struct UnboundProtocols {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer: Option<ZwlrLayerShellV1>,
}

impl UnboundProtocols {
    fn finalize(&mut self) -> Option<BoundProtocols> {
        if self.compositor.is_some() && self.shm.is_some() && self.layer.is_some() {
            Some(BoundProtocols {
                compositor: self.compositor.take().expect("Function guard is too weak"),
                shm: self.shm.take().expect("Function guard is too weak"),
                layer: self.layer.take().expect("Function guard is too weak"),
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoundProtocols {
    compositor: WlCompositor,
    shm: WlShm,
    layer: ZwlrLayerShellV1,
}

impl BoundProtocols {
    pub fn get_compositor(&self) -> &WlCompositor {
        &self.compositor
    }

    pub fn get_shm(&self) -> &WlShm {
        &self.shm
    }

    pub fn get_layer(&self) -> &ZwlrLayerShellV1 {
        &self.layer
    }
}

pub struct WaylandState {
    pub unbound: UnboundProtocols,
    pub bound: Option<BoundProtocols>,
    pub surface_creators: HashMap<ObjectId, UninitSurface>,
    pub surface_links: HashMap<ObjectId, Surface>,
    pub gl: GlAbstraction,
}

impl WaylandState {
    pub fn new(display: &WlDisplay) -> WaylandState {
        WaylandState {
            unbound: UnboundProtocols::default(),
            bound: None,
            surface_creators: HashMap::new(),
            surface_links: HashMap::new(),
            gl: GlAbstraction::new(display).expect("Unable to abstract GL"),
        }
    }

    pub fn handle_events(
        &mut self,
        event_queue: &mut EventQueue<Self>,
    ) -> Result<(), DispatchError> {
        event_queue.blocking_dispatch(self)?;

        let ready: Vec<UninitSurface> = self
            .surface_creators
            .extract_if(|_, uninit| uninit.is_ready())
            .map(|(_, uninit)| uninit)
            .collect();
        for uninit in ready {
            uninit.finalize(self);
        }

        Ok(())
    }

    /// Start the creation of a surface (`ZwlrLayerShellV1`)
    ///
    /// Due to the nature of Wayland, the creation is not immediate and requires a roundtrip with
    /// the wayland server. The `ObjectId` returned by this function can be used to check if the
    /// surface creation has been finalized.
    pub fn create_surface_async(
        &mut self,
        width: u32,
        height: u32,
        layer: Layer,
        event_queue: &mut EventQueue<Self>,
    ) -> Option<ObjectId> {
        let queue_handle = event_queue.handle();
        UninitSurface::setup(width, height, layer, self, &queue_handle)
    }

    /// Start the creation of a surface (`ZwlrLayerShellV1`) and wait for its completion
    ///
    /// # Warning
    /// This function is VERY prone to deadlocks, only use it for quick debugging purposes
    pub fn create_surface_blocking(
        &mut self,
        width: u32,
        height: u32,
        layer: Layer,
        event_queue: &mut EventQueue<Self>,
    ) -> Option<ObjectId> {
        let queue_handle = event_queue.handle();
        let id = UninitSurface::setup(width, height, layer, self, &queue_handle)?;

        while !self.surface_links.contains_key(&id) {
            self.handle_events(event_queue).ok()?;
        }
        Some(id)
    }
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.unbound.compositor =
                        Some(proxy.bind::<WlCompositor, _, _>(name, version, qhandle, ()));
                    state.bound = state.unbound.finalize();
                }
                "wl_shm" => {
                    state.unbound.shm = Some(proxy.bind::<WlShm, _, _>(name, version, qhandle, ()));
                    state.bound = state.unbound.finalize();
                }
                "zwlr_layer_shell_v1" => {
                    state.unbound.layer =
                        Some(proxy.bind::<ZwlrLayerShellV1, _, _>(name, version, qhandle, ()));
                    state.bound = state.unbound.finalize();
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(WaylandState: ignore WlCompositor);
delegate_noop!(WaylandState: ignore WlShm);
delegate_noop!(WaylandState: ignore WlSurface);
delegate_noop!(WaylandState: ignore WlShmPool);
delegate_noop!(WaylandState: ignore WlBuffer);
delegate_noop!(WaylandState: ignore ZwlrLayerShellV1);
