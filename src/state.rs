use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wayland_client::{
    self, Connection, Dispatch, DispatchError, EventQueue,
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_display::WlDisplay,
        wl_registry::{self, WlRegistry},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

use crate::{
    gpu_surface::GlAbstraction,
    surface::{Surface, SurfaceId},
};

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
    pub pending_surfaces: HashSet<usize>,
    // pub surface_creators: HashMap<ObjectId, UninitSurface>,
    // pub surface_links: HashMap<ObjectId, Surface>,
    // pub surface_creation_callback: HashMap<ObjectId, Box<dyn FnOnce(&mut Self, ObjectId)>>,
    pub gl: GlAbstraction,
}

impl WaylandState {
    pub fn new(display: &WlDisplay) -> WaylandState {
        WaylandState {
            unbound: UnboundProtocols::default(),
            pending_surfaces: HashSet::new(),
            bound: None,
            gl: GlAbstraction::new(display).expect("Unable to abstract GL"),
        }
    }

    pub fn post_dispatch(
        &mut self,
        event_queue: &mut EventQueue<Self>,
    ) -> Result<(), DispatchError> {
        // todo!();
        // let ready: Vec<(ObjectId, UninitSurface)> = self
        //     .surface_creators
        //     .extract_if(|_, uninit| uninit.is_ready())
        //     .collect();
        // for (key, uninit) in ready {
        //     uninit.finalize(self);
        //
        //     println!("finalized {key:?}");
        //     if let Some(callback) = self.surface_creation_callback.remove(&key) {
        //         callback(self, key)
        //     }
        // }

        Ok(())
    }

    pub fn handle_events(
        &mut self,
        event_queue: &mut EventQueue<Self>,
    ) -> Result<(), DispatchError> {
        event_queue.roundtrip(self)?;

        self.post_dispatch(event_queue)
    }

    pub fn handle_events_blocking(
        &mut self,
        event_queue: &mut EventQueue<Self>,
    ) -> Result<(), DispatchError> {
        event_queue.blocking_dispatch(self)?;

        self.post_dispatch(event_queue)
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
