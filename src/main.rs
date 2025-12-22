use std::{
    collections::HashMap,
    hash::Hash,
    sync::{LazyLock, Mutex},
};

use memfd::Shm;
use wayland_client::{
    self, Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_registry::{self, WlRegistry},
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

// static SURFACE_LINK: LazyLock<Mutex<HashMap<u32, SurfaceLink>>> =
//     std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Default)]
struct UnboundProtocols {
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
struct BoundProtocols {
    compositor: WlCompositor,
    shm: WlShm,
    layer: ZwlrLayerShellV1,
}

#[derive(Debug, Default)]
struct WaylandState {
    unbound: UnboundProtocols,
    bound: Option<BoundProtocols>,
    active_creator: Option<SurfaceCreator>,
    surface_links: HashMap<ObjectId, Surface>,
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

    fn create_surface_blocking(
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

#[derive(Debug, Clone, Copy, Default)]
struct Margins {
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Sizes {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct SurfaceProperties {
    margins: Margins,
    anchor: Anchor,
    interactivity: KeyboardInteractivity,
    layer: Layer,
    sizes: Sizes,
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
struct Surface {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    pool: WlShmPool,
    shm: Shm,
    properties: SurfaceProperties,
}
// unsafe impl Sync for Surface {}
// unsafe impl Send for Surface {}

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

    pub fn get_properties(&self) -> SurfaceProperties {
        self.properties
    }
}

#[derive(Debug, Default)]
struct SurfaceCreator {
    properties: SurfaceProperties,
    surfaces: Option<(WlSurface, ZwlrLayerSurfaceV1)>,
    buffers: Option<(WlShmPool, WlBuffer)>,
    data: Option<Shm>,
}

impl SurfaceCreator {
    fn is_ready(&self) -> bool {
        self.buffers.is_some() && self.surfaces.is_some() && self.data.is_some()
    }

    fn create_surface(
        &mut self,
        width: u32,
        height: u32,
        layer: Layer,
        protocols: &BoundProtocols,
        queue_handle: &QueueHandle<WaylandState>,
    ) {
        let surface = protocols.compositor.create_surface(queue_handle, ());
        let layer_surface = protocols.layer.get_layer_surface(
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
    fn finalize(self, state: &mut WaylandState) -> Option<ObjectId> {
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
        data: &(),
        conn: &Connection,
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
                        let pool = protocols.shm.create_pool(shm.get_fd(), total_buffer_size as i32, qhandle, ());
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

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &WlRegistry,
        event: wl_registry::Event,
        data: &(),
        conn: &Connection,
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

fn redraw(surface: &mut Surface) {
    let data = surface.shm.data_mut();
    let mut index = 0;
    for y in 0..surface.properties.sizes.height {
        for x in 0..surface.properties.sizes.width {
            let (r, g, b) = match x {
                rx if rx <= surface.properties.sizes.width / 3 => (255, 0, 0),
                rx if rx <= surface.properties.sizes.width / 3 * 2 => (0, 255, 0),
                _ => (0, 0, 255),
            };

            let a = match y {
                0..25 => 120,
                25..50 => 20,
                50..75 => 0,
                75.. => 255,
            };

            data[index + 3] = a;
            data[index + 2] = r;
            data[index + 1] = g;
            data[index]     = b;

            index += 4;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wayland_state = WaylandState::default();

    let connection = Connection::connect_to_env()?;

    let display = connection.display();
    let mut event_queue = connection.new_event_queue();
    let queue_handle = event_queue.handle();

    display.get_registry(&queue_handle, ());

    event_queue.roundtrip(&mut wayland_state)?;

    let surface_id = wayland_state
        .create_surface_blocking(500, 100, Layer::Top, &mut event_queue)
        .unwrap_or(ObjectId::null());

    if let Some(surface) = wayland_state.surface_links.get_mut(&surface_id) {
        redraw(surface);
    }

    // if let Some(creator) =  {
    //     creator.
    // }

    // if let Some(creator) = wayland_state.get_surface_creator() {
    //     creator.create_surface(300, 300, &queue_handle);
    // } else {
    //     todo!("loop til creator?");
    // }

    println!("Entering dispatch loop");
    let mut top = 0;
    let mut w = 0;
    while display.is_alive() {
        if let Some(surface) = wayland_state.surface_links.get_mut(&surface_id) {
            surface.set_margin(Margins {
                top,
                right: 0,
                bottom: 0,
                left: 0,
            });
            surface.set_size(Sizes {
                width: 200 + w,
                height: 100,
            });
            redraw(surface);
            // surface.set_anchor(Anchor::Left);
            // let data = surface.shm.data_mut();
            // for i in (0..data.len()).step_by(4) {
            //     // ARGB
            //     data[i + 0] = 255;
            //     data[i + 1] = 0;
            //     data[i + 2] = 0;
            //     data[i + 3] = 255;
            // }
        }
        event_queue.blocking_dispatch(&mut wayland_state)?;
        top += 1;
        top %= 1300;
        w += 4;
        w %= 200;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("Exiting dispatch loop");

    Ok(())
}
