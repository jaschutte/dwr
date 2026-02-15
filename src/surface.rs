use std::rc::*;
use std::{num::NonZero, pin::Pin};

use glcore::{GL_1_0_g, GLCore, GLCoreError};
use mlua::FromLua;
use wayland_client::{
    self, Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    protocol::{
        wl_buffer::WlBuffer, wl_shm::Format, wl_shm_pool::WlShmPool, wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};

use crate::{gpu_surface::GpuInterface, state::WaylandState};
use crate::error::Error;

const BUFFER_NAMESPACE: &str = "DWR_BUF";

pub type Anchor = zwlr_layer_surface_v1::Anchor;
pub type Layer = zwlr_layer_shell_v1::Layer;
pub type KeyboardInteractivity = zwlr_layer_surface_v1::KeyboardInteractivity;

#[derive(Debug, Clone, Copy, Default)]
pub struct Margins {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Sizes {
    pub width: NonZero<u32>,
    pub height: NonZero<u32>,
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            width: unsafe { NonZero::new_unchecked(100) },
            height: unsafe { NonZero::new_unchecked(100) },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceProperties {
    pub margins: Margins,
    pub anchor: Anchor,
    pub interactivity: KeyboardInteractivity,
    pub layer: Layer,
    pub size: Sizes,
}

impl Default for SurfaceProperties {
    fn default() -> Self {
        Self {
            margins: Default::default(),
            anchor: Anchor::Top,
            interactivity: KeyboardInteractivity::None,
            layer: Layer::Top,
            size: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct SurfaceInterfaces {
    wayland_surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    gpu_interface: GpuInterface,
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceState {
    prerender_roundtrip: bool,
    postrender_roundtrip: bool,
    ready: bool,
}

#[derive(Debug)]
pub struct Surface {
    properties: SurfaceProperties,
    interfaces: SurfaceInterfaces,
}

pub type SurfaceId = ObjectId;
pub type SurfaceReference = Weak<Surface>;

struct UnconfiguredSurface<T: Clone> {
    wayland_surface: WlSurface,
    properties: SurfaceProperties,
    callback: SurfaceCallback<T>,
    udata: T,
}
pub type SurfaceCallback<T> = fn(Surface, T);

impl Surface {
    pub fn get_id(&self) -> SurfaceId {
        self.interfaces.layer_surface.id()
    }

    pub fn render(
        &mut self,
        render: fn(glcore::GLCore) -> Result<(), GLCoreError>,
    ) -> Result<(), Error> {
        render(self.interfaces.gpu_interface.get_renderer()).map_err(|err| Error::from(err))?;
        self.interfaces.gpu_interface.swap_buffers()?;
        Ok(())
    }

    pub fn set_margin(&mut self, margins: Margins) {
        self.interfaces.layer_surface.set_margin(
            margins.top,
            margins.right,
            margins.bottom,
            margins.left,
        );
        self.properties.margins = margins;
        self.interfaces.wayland_surface.commit();
    }

    pub fn set_size(&mut self, sizes: Sizes) {
        self.interfaces
            .layer_surface
            .set_size(sizes.width.get(), sizes.height.get());
        self.interfaces
            .gpu_interface
            .resize(sizes.width, sizes.height);
        self.properties.size = sizes;
        self.interfaces.wayland_surface.commit();
    }

    pub fn set_layer(&mut self, layer: Layer) {
        self.interfaces.layer_surface.set_layer(layer);
        self.properties.layer = layer;
        self.interfaces.wayland_surface.commit();
    }

    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.interfaces.layer_surface.set_anchor(anchor);
        self.properties.anchor = anchor;
        self.interfaces.wayland_surface.commit();
    }

    pub fn create<T: Clone + Send + Sync + 'static>(
        properties: SurfaceProperties,
        state: &mut WaylandState,
        queue_handle: &QueueHandle<WaylandState>,
        callback: SurfaceCallback<T>,
        udata: T,
    ) -> Option<Surface> {
        let protocols = state.bound.as_ref()?;

        let wayland_surface = protocols.get_compositor().create_surface(queue_handle, ());
        let layer_protocol = protocols.get_layer().to_owned();
        let layer_surface = layer_protocol.get_layer_surface(
            &wayland_surface,
            None,
            properties.layer,
            BUFFER_NAMESPACE.into(),
            queue_handle,
            UnconfiguredSurface {
                wayland_surface: wayland_surface.clone(),
                properties,
                callback,
                udata,
            },
        );
        layer_surface.set_margin(
            properties.margins.top,
            properties.margins.right,
            properties.margins.bottom,
            properties.margins.left,
        );
        layer_surface.set_anchor(properties.anchor);
        layer_surface.set_keyboard_interactivity(properties.interactivity);
        layer_surface.set_size(properties.size.width.get(), properties.size.height.get());

        state
            .pending_surfaces
            .insert(callback as *const SurfaceCallback<T> as usize);
        wayland_surface.commit();

        None
    }
}

impl<T: Clone> Dispatch<ZwlrLayerSurfaceV1, UnconfiguredSurface<T>> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        unconfigured: &UnconfiguredSurface<T>,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                proxy.ack_configure(serial);

                let callback_ptr = unconfigured.callback as *const SurfaceCallback<T> as usize;
                let is_pending = state.pending_surfaces.contains(&callback_ptr);
                if is_pending
                    && let Ok(gpu_interface) = GpuInterface::new(
                        &state.gl,
                        &unconfigured.wayland_surface,
                        unconfigured.properties.size.width,
                        unconfigured.properties.size.height,
                    )
                {
                    let layer_surface = proxy.to_owned();
                    let surface = Surface {
                        properties: unconfigured.properties,
                        interfaces: SurfaceInterfaces {
                            wayland_surface: unconfigured.wayland_surface.clone(),
                            layer_surface,
                            gpu_interface,
                        },
                    };

                    state.pending_surfaces.remove(&callback_ptr);
                    (unconfigured.callback)(surface, unconfigured.udata.clone())
                }
            }
            _ => todo!("Implement missing event (closed event, prob)"),
        }
    }
}
