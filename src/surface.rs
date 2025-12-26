use std::num::NonZero;

use glcore::GLCore;
use memfd::Shm;
use wayland_client::{
    self, Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    protocol::{
        wl_buffer::WlBuffer, wl_shm::Format, wl_shm_pool::WlShmPool, wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event as LayerEvent;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::Layer,
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use crate::{gpu_surface::GpuSurface, state::WaylandState};

const BUFFER_NAMESPACE: &str = "DWR_BUF";

#[derive(Debug, Clone, Copy, Default)]
pub struct Margins {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Sizes {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SurfaceProperties {
    pub margins: Margins,
    pub anchor: Anchor,
    pub interactivity: KeyboardInteractivity,
    pub layer: Layer,
    pub sizes: Sizes,
}

impl Default for SurfaceProperties {
    fn default() -> Self {
        Self {
            margins: Default::default(),
            anchor: Anchor::Top,
            interactivity: KeyboardInteractivity::None,
            layer: Layer::Top,
            sizes: Default::default(),
        }
    }
}

pub struct Surface {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    pool: WlShmPool,
    gpu_surface: GpuSurface,
    shm: Shm,
    properties: SurfaceProperties,
}

impl Surface {
    pub fn get_renderer(&self) -> GLCore {
        self.gpu_surface.get_renderer()
    }

    pub fn render(
        &mut self,
        render: fn(glcore::GLCore) -> Result<(), glcore::GLCoreError>,
    ) -> Result<(), glcore::GLCoreError> {
        render(self.get_renderer())
    }

    pub fn swap_buffers(&mut self) -> Result<(), glutin::error::Error> {
        self.gpu_surface.swap_buffers()
    }

    pub fn set_margin(&mut self, margins: Margins) {
        self.layer_surface
            .set_margin(margins.top, margins.right, margins.bottom, margins.left);
        self.properties.margins = margins;
        self.surface.commit();
    }

    pub fn set_size(&mut self, sizes: Sizes) {
        self.layer_surface.set_size(sizes.width, sizes.height);
        self.surface.commit();
    }

    pub fn set_layer(&mut self, layer: Layer) {
        self.layer_surface.set_layer(layer);
        self.surface.commit();
    }

    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.layer_surface.set_anchor(anchor);
        self.properties.anchor = anchor;
        self.surface.commit();
    }

    pub fn set_keyboard_interactivity(&mut self, keyboard_interactivity: KeyboardInteractivity) {
        self.layer_surface
            .set_keyboard_interactivity(keyboard_interactivity);
        self.properties.interactivity = keyboard_interactivity;
        self.surface.commit();
    }

    pub fn get_pixel_buffer(&self) -> &[u8] {
        self.shm.data()
    }

    pub fn get_pixel_buffer_mut(&mut self) -> &mut [u8] {
        self.shm.data_mut()
    }

    pub fn set_properties(&mut self, mut props: SurfaceProperties) {
        let new_sizes = props.sizes;
        // We should only update the size property as soon as we reallocated the memory
        // This is done automatically later at the realloc code
        // Until then, assume the old sizes
        props.sizes = self.properties.sizes;
        self.properties = props;
        self.layer_surface.set_margin(
            self.properties.margins.top,
            self.properties.margins.right,
            self.properties.margins.bottom,
            self.properties.margins.left,
        );
        self.layer_surface.set_anchor(self.properties.anchor);
        self.layer_surface
            .set_keyboard_interactivity(self.properties.interactivity);
        self.layer_surface
            .set_size(new_sizes.width, new_sizes.height);
        self.surface.commit();
    }

    pub fn get_properties(&self) -> &SurfaceProperties {
        &self.properties
    }
}

pub struct UninitSurface {
    properties: SurfaceProperties,
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    gpu_surface: Option<GpuSurface>,
    buffers: Option<(WlShmPool, WlBuffer)>,
    data: Option<Shm>,
}

impl UninitSurface {
    pub fn is_ready(&self) -> bool {
        self.buffers.is_some() && self.data.is_some()
    }

    /// Starts the creation of a Wayland native surface, in specific a `ZwlrLayerSurfaceV1`
    ///
    /// Creating a surface in Wayland is async, it requires a roundtrip with the server and
    /// therefore cannot be done directly.
    ///
    /// TODO: explain `UninitSurface` -> `Surface`
    pub fn setup(
        width: u32,
        height: u32,
        layer: Layer,
        state: &mut WaylandState,
        queue_handle: &QueueHandle<WaylandState>,
    ) -> Option<ObjectId> {
        let protocols = state.bound.as_ref()?;

        let surface = protocols.get_compositor().create_surface(queue_handle, ());
        let layer_surface = protocols.get_layer().get_layer_surface(
            &surface,
            None,
            layer,
            BUFFER_NAMESPACE.into(),
            queue_handle,
            (),
        );
        let layer_id = layer_surface.id().clone();

        let mut uninit_surface = UninitSurface {
            properties: SurfaceProperties::default(),
            surface,
            layer_surface,
            gpu_surface: None,
            buffers: None,
            data: None,
        };
        uninit_surface.properties.sizes = Sizes { width, height };

        uninit_surface.layer_surface.set_margin(
            uninit_surface.properties.margins.top,
            uninit_surface.properties.margins.right,
            uninit_surface.properties.margins.bottom,
            uninit_surface.properties.margins.left,
        );
        uninit_surface
            .layer_surface
            .set_anchor(uninit_surface.properties.anchor);
        uninit_surface
            .layer_surface
            .set_keyboard_interactivity(uninit_surface.properties.interactivity);
        uninit_surface.layer_surface.set_size(
            uninit_surface.properties.sizes.width,
            uninit_surface.properties.sizes.height,
        );
        uninit_surface.surface.commit();

        state
            .surface_creators
            .insert(layer_id.clone(), uninit_surface);
        Some(layer_id)
    }

    /// Make sure `is_ready()` returns true!
    pub fn finalize(self, state: &mut WaylandState) -> Option<ObjectId> {
        self.data
            .zip(self.buffers)
            .zip(self.gpu_surface)
            .map(|((shm, (pool, _)), gpu_surface)| Surface {
                shm,
                surface: self.surface,
                layer_surface: self.layer_surface,
                gpu_surface,
                pool,
                properties: self.properties,
            })
            .map(|surface| {
                let id = surface.layer_surface.id();
                state.surface_links.insert(id.clone(), surface);
                id
            })
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        match event {
            LayerEvent::Configure {
                serial,
                width,
                height,
            } => {
                proxy.ack_configure(serial);

                // The server may give us 0, 0
                // This means 'you decide', we default to 100x100
                // Maybe change this to whatever the surface has?
                let nn_width: NonZero<u32> = width
                    .try_into()
                    .unwrap_or(unsafe { NonZero::new_unchecked(1) });
                let nn_height: NonZero<u32> = height
                    .try_into()
                    .unwrap_or(unsafe { NonZero::new_unchecked(1) });
                let width = u32::from(nn_width);
                let height = u32::from(nn_height);

                let bytes_per_pixel = 4;
                let stride = width * bytes_per_pixel;
                let num_of_frames = 2;
                let total_buffer_size = height * stride * num_of_frames;

                if let Some(linked) = state.surface_links.get_mut(&proxy.id())
                    && let Ok(_) = linked.shm.resize(total_buffer_size as usize)
                {
                    linked.gpu_surface.resize(nn_width, nn_height);

                    linked.properties.sizes.height = height;
                    linked.properties.sizes.width = width;

                    let buffer = linked.pool.create_buffer(
                        0,
                        width as i32,
                        height as i32,
                        stride as i32,
                        Format::Argb8888,
                        qhandle,
                        (),
                    );
                    linked.gpu_surface.resize(nn_width, nn_height);
                    linked.surface.attach(Some(&buffer), 0, 0);
                    linked.surface.damage(0, 0, width as i32, height as i32);
                    linked.surface.commit();
                }

                if let Some(linked) = state.surface_creators.get_mut(&proxy.id())
                    && let Some(protocols) = &state.bound
                    && let Ok(shm) = Shm::new(total_buffer_size as usize)
                    && let Ok(egl_surface) =
                        GpuSurface::new(&state.gl, &linked.surface, nn_width, nn_height)
                {
                    let pool = protocols.get_shm().create_pool(
                        shm.get_fd(),
                        total_buffer_size as i32,
                        qhandle,
                        (),
                    );
                    let buffer = pool.create_buffer(
                        0,
                        width as i32,
                        height as i32,
                        stride as i32,
                        Format::Argb8888,
                        qhandle,
                        (),
                    );

                    linked.gpu_surface = Some(egl_surface);
                    linked.surface.attach(Some(&buffer), 0, 0);
                    linked.surface.damage(0, 0, width as i32, height as i32);
                    linked.surface.commit();

                    linked.buffers = Some((pool, buffer));
                    linked.data = Some(shm);
                }
            }
            LayerEvent::Closed => todo!(),
            _ => todo!(),
        }
    }
}
