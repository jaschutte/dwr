use std::{
    cell::{Ref, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};

use mlua::{
    Error as LError, ExternalResult, FromLua, FromLuaMulti, Lua, Result as LResult, UserData,
};
use wayland_backend::client::ObjectId;

use crate::{
    state::WaylandState,
    surface::{Margins, Sizes, Surface},
};

#[derive(Clone)]
pub struct LuaSurfaceReference {
    id: ObjectId,
    state: Rc<RefCell<WaylandState>>,
}

impl LuaSurfaceReference {
    pub fn new(id: ObjectId, state: Rc<RefCell<WaylandState>>) -> LuaSurfaceReference {
        LuaSurfaceReference { id, state }
    }

    fn set_margin(_: &Lua, reference: &mut Self, margins: Margins) -> LResult<()> {
        let mut state = reference.state.try_borrow_mut().into_lua_err()?;
        let surface = state
            .surface_links
            .get_mut(&reference.id)
            .ok_or(LError::MemoryError(
                "Surface reference invalid, this should never be possible".into(),
            ))?;
        surface.set_margin(margins);
        Ok(())
    }

    fn set_size(_: &Lua, reference: &mut Self, sizes: Sizes) -> LResult<()> {
        let mut state = reference.state.try_borrow_mut().into_lua_err()?;
        let surface = state
            .surface_links
            .get_mut(&reference.id)
            .ok_or(LError::MemoryError(
                "Surface reference invalid, this should never be possible".into(),
            ))?;
        surface.set_size(sizes);
        Ok(())
    }

    fn demo_render(_: &Lua, reference: &mut Self, _: ()) -> LResult<()> {
        let mut state = reference.state.try_borrow_mut().into_lua_err()?;
        super::entry::WaylandClient::render_test(&mut state, &reference.id);
        Ok(())
    }
}

impl UserData for LuaSurfaceReference {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set_margin", LuaSurfaceReference::set_margin);
        methods.add_method_mut("set_size", LuaSurfaceReference::set_size);
        methods.add_method_mut("demo_render", LuaSurfaceReference::demo_render);
    }
}

// impl FromLuaMulti for LuaSurfaceReference {
//     fn from_lua_multi(values: mlua::MultiValue, lua: &mlua::Lua) -> mlua::Result<Self> {
//         todo!()
//     }
// }

impl FromLua for Margins {
    fn from_lua(value: mlua::Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
        let table = value.as_table().ok_or(LError::ToLuaConversionError {
            from: value.type_name().to_string(),
            to: "{ top = <number>, right = <number>, left = <number>, bottom = <number> }",
            message: None,
        })?;
        let missing_entry = |name: &'static str| {
            move |_| {
                LError::RuntimeError(format!("creating Margins type failed, missing key: {name}"))
            }
        };

        Ok(Margins {
            top: table.get("top").map_err(missing_entry("top"))?,
            left: table.get("left").map_err(missing_entry("left"))?,
            right: table.get("right").map_err(missing_entry("right"))?,
            bottom: table.get("bottom").map_err(missing_entry("bottom"))?,
        })
    }
}

impl FromLua for Sizes {
    fn from_lua(value: mlua::Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
        let table = value.as_table().ok_or(LError::ToLuaConversionError {
            from: value.type_name().to_string(),
            to: "{ width = <number>, height = <number> }",
            message: None,
        })?;
        let missing_entry = |name: &'static str| {
            move |_| {
                LError::RuntimeError(format!("creating Margins type failed, missing key: {name}"))
            }
        };

        Ok(Sizes {
            width: table.get("width").map_err(missing_entry("width"))?,
            height: table.get("height").map_err(missing_entry("height"))?,
        })
    }
}
