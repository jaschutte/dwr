use std::{
    cell::{Ref, RefCell},
    num::NonZero,
    rc::{Rc, Weak},
    sync::Arc,
};

use mlua::{
    Error as LError, ExternalResult, FromLua, FromLuaMulti, Lua, Result as LResult, UserData,
};
use wayland_backend::client::ObjectId;

use crate::{
    lua::entry::LuaAnchor,
    state::WaylandState,
    surface::{Anchor, Margins, Sizes, Surface},
};

#[derive(Debug)]
pub struct LuaSurface {
    surface: Surface,
}

impl LuaSurface {
    pub fn new(surface: Surface) -> LuaSurface {
        LuaSurface { surface }
    }

    /// Sets the margins of the [`LuaSurface`]
    ///
    /// Positions the `LuaSurface` relative to the anchor point, set by [`LuaSurface::set_anchor`]
    ///
    /// When the anchor point is set to `Anchor::Top | Anchor::Left`, this can essentially set the
    /// window position relative to the screen
    ///
    /// For more information, look at the [Wayland protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1#zwlr_layer_surface_v1:request:set_margin)
    fn set_margin(_: &Lua, reference: &mut Self, margins: Margins) -> LResult<()> {
        reference.surface.set_margin(margins);

        Ok(())
    }

    /// Sets the anchor point of the [`LuaSurface`]
    ///
    /// The anchor is which edge(s) of the screen the window position (=margin) will be relative to
    ///
    /// Setting this to `Anchor::Top | Anchor::Left` will make surface be able to position itself
    /// as if one were using screen coordinates
    ///
    /// For more information, look at the [Wayland protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1#zwlr_layer_surface_v1:request:set_anchor)
    fn set_anchor(_: &Lua, reference: &mut Self, anchor: LuaAnchor) -> LResult<()> {
        reference.surface.set_anchor(anchor.into());
        Ok(())
    }

    fn set_size(_: &Lua, reference: &mut Self, sizes: Sizes) -> LResult<()> {
        reference.surface.set_size(sizes);
        Ok(())
    }

    fn demo_render(_: &Lua, reference: &mut Self, _: ()) -> LResult<()> {
        super::entry::WaylandClient::render_test(&mut reference.surface);
        Ok(())
    }
}

impl UserData for LuaSurface {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set_margin", LuaSurface::set_margin);
        methods.add_method_mut("set_anchor", LuaSurface::set_anchor);
        methods.add_method_mut("set_size", LuaSurface::set_size);
        methods.add_method_mut("demo_render", LuaSurface::demo_render);
    }
}

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

        let width32: u32 = table.get("width").map_err(missing_entry("width"))?;
        let height32: u32 = table.get("height").map_err(missing_entry("height"))?;
        let width = NonZero::try_from(width32)
            .map_err(|_| LError::RuntimeError(format!("Width may not be zero")))?;
        let height = NonZero::try_from(height32)
            .map_err(|_| LError::RuntimeError(format!("Width may not be zero")))?;

        Ok(Sizes { width, height })
    }
}
