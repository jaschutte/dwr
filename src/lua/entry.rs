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

use super::rendering::LuaSurfaceReference;
use crate::{opengl::types::GlResult, state::WaylandState};

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

    pub fn render_test(state: &mut WaylandState, surface_id: &ObjectId) {
        let surface = state.surface_links.get_mut(surface_id).unwrap();
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
        let _ = surface.swap_buffers();
    }

    fn create_surface(
        _: &Lua,
        client: &mut Self,
        (w, h, callback): (u32, u32, Function),
    ) -> LResult<()> {
        let surface_id = client
            .state
            .borrow_mut()
            .create_surface_async(w, h, Layer::Top, &mut client.event_queue)
            .unwrap_or(ObjectId::null());

        let rc_state = client.state.clone();
        client.state.borrow_mut().surface_creation_callback.insert(
            surface_id,
            Box::new(move |state, surface_id| {
                WaylandClient::render_test(state, &surface_id);

                let reference = LuaSurfaceReference::new(surface_id, rc_state);
                let _ = callback.call::<()>(reference);
            }),
        );

        Ok(())
    }

    fn render(_: &Lua, client: &mut Self, _: ()) -> LResult<()> {
        let mut state = client.state.borrow_mut();
        state
            .handle_events(&mut client.event_queue)
            .into_lua_err()?;
        std::thread::sleep(std::time::Duration::from_millis(16));

        Ok(())
    }
}

impl UserData for WaylandClient {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("is_alive", WaylandClient::is_alive);
        methods.add_method_mut("create_surface", WaylandClient::create_surface);
        methods.add_method_mut("render", WaylandClient::render);
    }
}

#[mlua::lua_module]
fn dwr(lua: &Lua) -> LResult<Table> {
    let exports = lua.create_table()?;
    exports.set("create_client", lua.create_function(WaylandClient::init)?)?;
    Ok(exports)
}
