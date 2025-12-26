use std::ffi::c_void;

use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_3_0_g};
use wayland_backend::client::ObjectId;
use wayland_client::{self, Connection, Proxy};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;

use crate::{
    shaders::{Shader, ShaderBundle},
    state::WaylandState,
    surface::Margins,
};
mod gpu_surface;
mod opengl;
mod shaders;
mod state;
mod surface;

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
                    let shader_program = ShaderBundle::new_from_files(
                        graphics,
                        "src/shaders/flat_color.vert",
                        "src/shaders/flat_color.frag",
                    )?
                    .link()?;
                    shader_program.use_program()?;

                    graphics.glClearColor(1.0, 0.0, 0.0, 1.0)?;
                    graphics.glClear(glcore::GL_COLOR_BUFFER_BIT | glcore::GL_DEPTH_BUFFER_BIT)?;

                    let vertices: [f32; 9] = [0.5, 0.5, 0.0, 0.5, -0.5, 0.0, -0.5, 0.5, 0.0];

                    // VAO
                    let mut vertex_attributes = 0;
                    graphics.glGenVertexArrays(1, &mut vertex_attributes)?;
                    graphics.glBindVertexArray(vertex_attributes)?;

                    // Copy over data to a buffer
                    let mut vertex_buffer = 0;
                    graphics.glGenBuffers(1, &mut vertex_buffer)?;
                    graphics.glBindBuffer(glcore::GL_ARRAY_BUFFER, vertex_buffer)?;
                    graphics.glBufferData(
                        glcore::GL_ARRAY_BUFFER,
                        std::mem::size_of::<[f32; 9]>(),
                        vertices.as_ptr() as *mut c_void,
                        glcore::GL_STATIC_DRAW,
                    )?;

                    // Vertex
                    graphics.glEnableVertexAttribArray(0)?;
                    graphics.glVertexAttribPointer(
                        0,
                        3,
                        glcore::GL_FLOAT,
                        glcore::GL_FALSE as u8,
                        0,
                        std::ptr::null(),
                    )?;

                    let color_index = shader_program.locate_uniform(c"color")?;
                    graphics.glUniform4f(color_index, 0.0, 0.8, 1.0, 1.0)?;

                    graphics.glDrawArrays(glcore::GL_TRIANGLES, 0, 3)?;
                    graphics.glDisableVertexAttribArray(0)?;

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
