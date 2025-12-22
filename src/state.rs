use std::collections::HashMap;

use wayland_client::{
    self, Connection, Dispatch, EventQueue,
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};

use crate::layer::{Surface, SurfaceCreator};

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

#[derive(Debug, Default)]
pub struct WaylandState {
    pub unbound: UnboundProtocols,
    pub bound: Option<BoundProtocols>,
    pub active_creator: Option<SurfaceCreator>,
    pub surface_links: HashMap<ObjectId, Surface>,
}

impl WaylandState {
    fn init_creator(&mut self) -> Option<(&mut SurfaceCreator, &BoundProtocols)> {
        if self.active_creator.is_some() {
            return None;
        }
        self.bound.as_ref()?;
        self.active_creator = Some(SurfaceCreator::default());
        self.active_creator.as_mut().zip(self.bound.as_ref())
    }

    pub fn create_surface_blocking(
        &mut self,
        width: u32,
        height: u32,
        layer: Layer,
        event_queue: &mut EventQueue<Self>,
    ) -> Option<ObjectId> {
        let queue_handle = event_queue.handle();

        if let Some((creator, protocols)) = self.init_creator() {
            creator.create_surface(width, height, layer, protocols, &queue_handle);
        }

        while self
            .active_creator
            .as_ref()
            .map(|c| !c.is_ready())
            .unwrap_or(false)
        {
            event_queue.blocking_dispatch(self).ok()?;
        }

        if let Some(creator) = self.active_creator.take() {
            creator.finalize(self)
        } else {
            None
        }
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
