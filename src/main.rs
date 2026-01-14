use wayland_backend::client::ObjectId;
use wayland_client::{self, Connection, Proxy};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;

use crate::{
    opengl::{
        highlevel::{ElementsMode, SimpleGL},
        shaders::builtin,
        types::{OwnedVec2Array, Vec2, Vec4},
    },
    state::WaylandState,
    surface::Margins,
};
mod gpu_surface;
mod opengl;
mod state;
mod surface;
mod lua;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;

    let display = connection.display();
    let mut event_queue = connection.new_event_queue();
    let queue_handle = event_queue.handle();

    display.get_registry(&queue_handle, ());

    let mut wayland_state = WaylandState::new(&display);
    event_queue.roundtrip(&mut wayland_state)?;

    let surface_id = wayland_state
        .create_surface_async(500, 300, Layer::Top, &mut event_queue)
        .unwrap_or(ObjectId::null());

    let mut has_surface = false;
    while display.is_alive() {
        wayland_state.handle_events(&mut event_queue)?;

        if let Some(surface) = wayland_state.surface_links.get_mut(&surface_id) {
            if !has_surface {
                has_surface = true;

                surface.set_margin(Margins {
                    top: 100,
                    right: 0,
                    bottom: 0,
                    left: 0,
                });

                let _ = surface.render(|graphics| {
                    let gl = SimpleGL::new(graphics);
                    let shader_program = gl
                        // .new_builtin_shader(builtin::FlatColor)?
                        .new_builtin_shader(builtin::QuadColor)?
                        .use_program()?;

                    let gl = gl.with_shader(shader_program);
                    gl.clear(0.2, 0.1, 0.0, 1.0)?;

                    shader_program.set_color(Vec4::new(0.0, 0.0, 1.0, 1.0))?;
                    gl.draw_polygon(
                        ElementsMode::LineLoop,
                        OwnedVec2Array::new(vec![
                            Vec2::new(-0.5, 0.5),
                            Vec2::new(0.5, 0.5),
                            Vec2::new(0.5, -0.5),
                        ]),
                    )?;

                    shader_program.set_color(Vec4::new(0.0, 1.0, 0.5, 1.0))?;
                    gl.draw_polygon(
                        ElementsMode::LineLoop,
                        OwnedVec2Array::new(vec![
                            Vec2::new(-0.5, 0.5),
                            Vec2::new(-0.5, -0.5),
                            Vec2::new(0.5, -0.5),
                        ]),
                    )?;

                    shader_program.set_color(Vec4::new(1.0, 0.0, 0.5, 1.0))?;
                    gl.draw_rectangle(Vec2::new(-0.2, -0.2), Vec2::new(0.4, 0.4))?;
                    shader_program.set_color(Vec4::new(1.0, 0.0, 0.5, 1.0))?;
                    gl.draw_rectangle(Vec2::new(-0.8, -0.8), Vec2::new(0.4, 0.4))?;

                    // shader_program.set_color(Vec4::new(1.0, 0.0, 0.5, 1.0))?;
                    // gl.draw_rectangle_generic(Vec2::new(0.3, 0.3), Vec2::new(0.4, 0.4))?;
                    // shader_program.set_color(Vec4::new(1.0, 0.0, 0.5, 1.0))?;
                    // gl.draw_rectangle_generic(Vec2::new(-0.3, 0.3), Vec2::new(0.4, 0.4))?;

                    Ok(())
                });
                surface.swap_buffers()?;
            }

            println!("frame with surface");
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
}
