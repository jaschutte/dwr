use std::ffi::{CStr, c_void};

use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_3_0_g, GLCore};
use glutin::surface::GlSurface;
use wayland_backend::client::ObjectId;
use wayland_client::{self, Connection, Proxy};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;

use crate::{
    state::WaylandState,
    surface::{Margins, Sizes, Surface},
};
mod gl;
mod state;
mod surface;

const VERTEX_SHADER_SOURCE: &CStr = c"#version 330 core
layout(location = 0) in vec3 pos;

void main() {
    gl_Position = vec4(pos.x, pos.y, pos.z, 1.0);
}";
const FRAGMENT_SHADER_SOURCE: &CStr = c"#version 330 core
out vec4 color;

void main() {
    color = vec4(0.0f, 1.0f, 0.2f, 1.0f);
}";

fn redraw(surface: &mut Surface) {
    let properties = *surface.get_properties();
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

#[derive(Debug, Clone, Copy)]
enum ProgramValidation {
    Vertex,
    Fragment,
    Linking,
}

impl ProgramValidation {
    fn label(self) -> &'static str {
        match self {
            ProgramValidation::Vertex => "compiling vertex",
            ProgramValidation::Fragment => "compiling fragment",
            ProgramValidation::Linking => "linking shaders",
        }
    }

    fn pname(self) -> u32 {
        match self.is_program() {
            true => glcore::GL_LINK_STATUS,
            false => glcore::GL_COMPILE_STATUS,
        }
    }

    fn is_program(self) -> bool {
        match self {
            ProgramValidation::Vertex => false,
            ProgramValidation::Fragment => false,
            ProgramValidation::Linking => true,
        }
    }
}

fn gl_validate_program(
    graphics: &mut GLCore,
    shader_or_program: u32,
    validate_type: ProgramValidation,
) -> Result<(), glcore::GLCoreError> {
    let mut shader_status = 0;
    let mut shader_status_len = 0;
    let pname = validate_type.pname();

    match validate_type.is_program() {
        true => {
            graphics.glGetProgramiv(shader_or_program, pname, &mut shader_status)?;
            graphics.glGetProgramiv(
                shader_or_program,
                glcore::GL_INFO_LOG_LENGTH,
                &mut shader_status_len,
            )?;
        }
        false => {
            graphics.glGetShaderiv(shader_or_program, pname, &mut shader_status)?;
            graphics.glGetShaderiv(
                shader_or_program,
                glcore::GL_INFO_LOG_LENGTH,
                &mut shader_status_len,
            )?;
        }
    }
    if shader_status_len > 0 {
        println!("Failed {} ({shader_status}):", validate_type.label());
        let mut log: [glcore::GLchar; 512] = [0; 512];
        match validate_type.is_program() {
            true => {
                graphics.glGetProgramInfoLog(
                    shader_or_program,
                    512,
                    std::ptr::null_mut(),
                    log.as_mut_ptr(),
                )?;
            }
            false => {
                graphics.glGetShaderInfoLog(
                    shader_or_program,
                    512,
                    std::ptr::null_mut(),
                    log.as_mut_ptr(),
                )?;
            }
        }
        let log_str: Vec<u8> = log
            .into_iter()
            .take(shader_status_len as usize)
            .map(|byte| byte as u8)
            .collect();
        println!(
            "-> {}",
            str::from_utf8(&log_str).unwrap_or("Failed retrieving error log")
        );
        Err(glcore::GLCoreError::UnknownError((
            0,
            "Shader failed compilation",
        )))
    } else {
        Ok(())
    }
}

fn gl_load_shaders(
    graphics: &mut GLCore,
    vertex: &CStr,
    fragment: &CStr,
) -> Result<u32, glcore::GLCoreError> {
    let shader_program = graphics.glCreateProgram()?;

    let vertex_shaders = [vertex.as_ptr()];
    let fragment_shaders = [fragment.as_ptr()];

    let vertex_shader = graphics.glCreateShader(glcore::GL_VERTEX_SHADER)?;
    let fragment_shader = graphics.glCreateShader(glcore::GL_FRAGMENT_SHADER)?;

    graphics.glShaderSource(vertex_shader, 1, vertex_shaders.as_ptr(), std::ptr::null())?;
    graphics.glCompileShader(vertex_shader)?;
    gl_validate_program(graphics, vertex_shader, ProgramValidation::Vertex)?;

    graphics.glShaderSource(
        fragment_shader,
        1,
        fragment_shaders.as_ptr(),
        std::ptr::null(),
    )?;
    graphics.glCompileShader(fragment_shader)?;
    gl_validate_program(graphics, fragment_shader, ProgramValidation::Fragment)?;

    graphics.glAttachShader(shader_program, vertex_shader)?;
    graphics.glAttachShader(shader_program, fragment_shader)?;
    graphics.glLinkProgram(shader_program)?;
    gl_validate_program(graphics, shader_program, ProgramValidation::Linking)?;

    graphics.glDetachShader(shader_program, vertex_shader)?;
    graphics.glDetachShader(shader_program, fragment_shader)?;
    graphics.glDeleteShader(vertex_shader)?;
    graphics.glDeleteShader(fragment_shader)?;

    Ok(shader_program)
}

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
                    let shader_program =
                        gl_load_shaders(graphics, VERTEX_SHADER_SOURCE, FRAGMENT_SHADER_SOURCE)?;

                    graphics.glClearColor(1.0, 0.0, 0.0, 1.0)?;
                    graphics.glClear(glcore::GL_COLOR_BUFFER_BIT | glcore::GL_DEPTH_BUFFER_BIT)?;
                    graphics.glUseProgram(shader_program)?;

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
