use memfd::Shm;
use wayland_client::{
    self, Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    protocol::{
        wl_buffer::WlBuffer, wl_shm::Format, wl_shm_pool::WlShmPool, wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::Layer,
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use crate::state::{BoundProtocols, WaylandState};

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

#[derive(Debug)]
pub struct Surface {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    pool: WlShmPool,
    shm: Shm,
    properties: SurfaceProperties,
}

impl Surface {
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

#[derive(Debug, Default)]
pub struct SurfaceCreator {
    properties: SurfaceProperties,
    surfaces: Option<(WlSurface, ZwlrLayerSurfaceV1)>,
    buffers: Option<(WlShmPool, WlBuffer)>,
    data: Option<Shm>,
}

impl SurfaceCreator {
    pub fn is_ready(&self) -> bool {
        self.buffers.is_some() && self.surfaces.is_some() && self.data.is_some()
    }

    pub fn create_surface(
        &mut self,
        width: u32,
        height: u32,
        layer: Layer,
        protocols: &BoundProtocols,
        queue_handle: &QueueHandle<WaylandState>,
    ) {
        let surface = protocols.get_compositor().create_surface(queue_handle, ());
        let layer_surface = protocols.get_layer().get_layer_surface(
            &surface,
            None,
            layer,
            "testing".to_owned(),
            queue_handle,
            (),
        );
        self.properties.sizes = Sizes { width, height };

        layer_surface.set_margin(
            self.properties.margins.top,
            self.properties.margins.right,
            self.properties.margins.bottom,
            self.properties.margins.left,
        );
        layer_surface.set_anchor(self.properties.anchor);
        layer_surface.set_keyboard_interactivity(self.properties.interactivity);
        layer_surface.set_size(self.properties.sizes.width, self.properties.sizes.height);
        surface.commit();

        self.surfaces = Some((surface, layer_surface));
    }

    /// Make sure `is_ready()` returns true!
    pub fn finalize(self, state: &mut WaylandState) -> Option<ObjectId> {
        self.data
            .zip(self.surfaces)
            .zip(self.buffers)
            .map(|((shm, (surface, layer_surface)), (pool, _))| Surface {
                shm,
                surface,
                layer_surface,
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
            wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                proxy.ack_configure(serial);

                let bytes_per_pixel = 4;
                let stride = width * bytes_per_pixel;
                let num_of_frames = 1;
                let total_buffer_size = height * stride * num_of_frames;

                if let Some(linked) = state.surface_links.get_mut(&proxy.id())
                    && let Ok(_) = linked.shm.resize(total_buffer_size as usize) {
                        linked.properties.sizes.height = height;
                        linked.properties.sizes.width = width;

                        let buffer = linked.pool.create_buffer(0, width as i32, height as i32, stride as i32, Format::Argb8888, qhandle, ());
                        linked.surface.attach(Some(&buffer), 0, 0);
                        linked.surface.damage(0, 0, width as i32, height as i32);
                        linked.surface.commit();
                    }

                if let (Some(creator), Some(protocols)) = (&mut state.active_creator, &state.bound)
                    && let Some((surface, _)) = &mut creator.surfaces {
                    if let Ok(shm) = Shm::new(total_buffer_size as usize) {
                        let pool = protocols.get_shm().create_pool(shm.get_fd(), total_buffer_size as i32, qhandle, ());
                        let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, Format::Argb8888, qhandle, ());

                        surface.attach(Some(&buffer), 0, 0);
                        surface.damage(0, 0, width as i32, height as i32);
                        surface.commit();

                        creator.buffers = Some((pool, buffer));
                        creator.data = Some(shm);
                    }
                }
            },
            wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event::Closed => todo!(),
            _ => todo!(),
        }
    }
}
