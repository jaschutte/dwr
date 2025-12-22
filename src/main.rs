use wayland_client::{self, Connection, Proxy, backend::ObjectId};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;

use crate::{layer::{Margins, Sizes, Surface}, state::WaylandState};
mod layer;
mod state;

fn redraw(surface: &mut Surface) {
    let properties = surface.get_properties().clone();
    let data = surface.get_pixel_buffer_mut();
    let mut index = 0;
    for y in 0..properties.sizes.height {
        for x in 0..properties.sizes.width {
            let (r, g, b) = match x {
                rx if rx <= properties.sizes.width / 3 => (255, 0, 0),
                rx if rx <= properties.sizes.width / 3 * 2 => (0, 255, 0),
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
            data[index] = b;

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
