use std::{cell::RefCell, rc::Rc};

use glcore::GLCoreError;
use mlua::{
    Error as LError, ExternalResult, Function, IntoLua, Lua, Result as LResult, Table, UserData,
    UserDataMethods,
};
use wayland_backend::client::ObjectId;
use wayland_client::{
    Connection, DispatchError, EventQueue, Proxy, QueueHandle, protocol::wl_display::WlDisplay,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;

use super::rendering::LuaSurface;
use crate::{opengl::types::GlResult, state::WaylandState, surface::{Margins, Surface, SurfaceProperties}};

pub struct WaylandClient {
    connection: Connection,
    display: WlDisplay,
    event_queue: EventQueue<WaylandState>,
    queue_handle: QueueHandle<WaylandState>,
    state: Rc<RefCell<WaylandState>>,
}

impl WaylandClient {
    fn init(_: &Lua, _: ()) -> LResult<WaylandClient> {
        let connection = Connection::connect_to_env().into_lua_err()?;

        let display = connection.display();
        let mut event_queue = connection.new_event_queue();
        let queue_handle = event_queue.handle();

        display.get_registry(&queue_handle, ());

        let mut state = WaylandState::new(&display);
        event_queue.roundtrip(&mut state).into_lua_err()?;

        Ok(WaylandClient {
            connection,
            display,
            event_queue,
            queue_handle,
            state: Rc::new(state.into()),
        })
    }

    fn is_alive(_: &Lua, client: &Self, _: ()) -> LResult<bool> {
        Ok(client.display.is_alive())
    }

    pub fn render_test(surface: &mut Surface) {
        let _ = surface.render(|graphics| {
            let gl = crate::opengl::highlevel::SimpleGL::new(graphics);
            let shader_program = gl
                // .new_builtin_shader(builtin::FlatColor)?
                .new_builtin_shader(crate::opengl::shaders::builtin::QuadColor)?
                .use_program()?;

            let gl = gl.with_shader(shader_program);
            gl.clear(0.2, 0.1, 0.0, 1.0)?;

            shader_program.set_color(crate::opengl::types::Vec4::new(0.0, 0.0, 1.0, 1.0))?;
            gl.draw_polygon(
                crate::opengl::highlevel::ElementsMode::LineLoop,
                crate::opengl::types::OwnedVec2Array::new(vec![
                    crate::opengl::types::Vec2::new(-0.5, 0.5),
                    crate::opengl::types::Vec2::new(0.5, 0.5),
                    crate::opengl::types::Vec2::new(0.5, -0.5),
                ]),
            )?;

            shader_program.set_color(crate::opengl::types::Vec4::new(0.0, 1.0, 0.5, 1.0))?;
            gl.draw_polygon(
                crate::opengl::highlevel::ElementsMode::LineLoop,
                crate::opengl::types::OwnedVec2Array::new(vec![
                    crate::opengl::types::Vec2::new(-0.5, 0.5),
                    crate::opengl::types::Vec2::new(-0.5, -0.5),
                    crate::opengl::types::Vec2::new(0.5, -0.5),
                ]),
            )?;

            shader_program.set_color(crate::opengl::types::Vec4::new(1.0, 0.0, 0.5, 1.0))?;
            gl.draw_rectangle(
                crate::opengl::types::Vec2::new(-0.2, -0.2),
                crate::opengl::types::Vec2::new(0.4, 0.4),
            )?;
            shader_program.set_color(crate::opengl::types::Vec4::new(1.0, 0.0, 0.5, 1.0))?;
            gl.draw_rectangle(
                crate::opengl::types::Vec2::new(-0.8, -0.8),
                crate::opengl::types::Vec2::new(0.4, 0.4),
            )?;

            Ok(())
        });
    }

    fn is_busy(_: &Lua, client: &Self, _: ()) -> LResult<bool> {
        Ok(client.state.try_borrow().is_err())
    }

    fn create_surface(
        _: &Lua,
        client: &mut Self,
        _: (),
    ) -> LResult<Option<LuaSurface>> {
        let state = match client.state.try_borrow().ok() {
            Some(state) => state,
            None => return Ok(None),
        };
        let properties = SurfaceProperties::default();
        let surface = match Surface::create(properties, &state, &client.queue_handle) {
            Some(state) => state,
            None => return Ok(None),
        };

        Ok(Some(LuaSurface::new(surface)))
    }

    fn render(_: &Lua, client: &mut Self, _: ()) -> LResult<bool> {
        let mut state = match client.state.try_borrow_mut().ok() {
            Some(state) => state,
            None => return Ok(false),
        };
        state
            .handle_events(&mut client.event_queue)
            .into_lua_err()?;
        std::thread::sleep(std::time::Duration::from_millis(16));

        Ok(true)
    }
}

impl UserData for WaylandClient {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("is_alive", WaylandClient::is_alive);
        methods.add_method("is_busy", WaylandClient::is_busy);
        methods.add_method_mut("try_create_surface", WaylandClient::create_surface);
        methods.add_method_mut("try_render", WaylandClient::render);
    }
}

#[mlua::lua_module]
fn dwr(lua: &Lua) -> LResult<Table> {
    let exports = lua.create_table()?;
    exports.set("create_client", lua.create_function(WaylandClient::init)?)?;
    Ok(exports)
}
