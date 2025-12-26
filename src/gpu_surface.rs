use glcore::GLCore;
use glutin::config::{Api, GlConfig};
use glutin::context::{
    AsRawContext, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext,
};
use glutin::error::{Error as GlutError, ErrorKind as GlutErrorKind};
use glutin::prelude::NotCurrentGlContext;
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface};
use glutin::{
    config::ConfigTemplateBuilder,
    display::{Display, DisplayApiPreference},
    prelude::GlDisplay,
};
use raw_window_handle::{HasDisplayHandle, RawWindowHandle, WaylandWindowHandle};
// use speedy2d::GLRenderer;
use std::ffi::CString;
use std::num::NonZero;
use std::{ffi::c_void, ptr::NonNull};
use wayland_client::Proxy;
use wayland_client::protocol::wl_display::WlDisplay;
use wayland_client::protocol::wl_surface::WlSurface;

#[derive(Debug, Clone)]
pub struct GlAbstraction {
    display: Display,
}

impl GlAbstraction {
    pub fn new(wl_display: &WlDisplay) -> Result<Self, GlutError> {
        let binding = wl_display.backend().upgrade().unwrap();
        let raw_display_handle = match binding.display_handle() {
            Ok(handle) => handle,
            Err(_) => {
                return Err(GlutError::from(GlutErrorKind::BadDisplay));
            }
        }
        .as_raw();
        let display = unsafe { Display::new(raw_display_handle, DisplayApiPreference::Egl) }?;
        Ok(GlAbstraction { display })
    }

    pub fn get_display(&self) -> &Display {
        &self.display
    }

    pub fn create_context(&self, surface: &WlSurface) -> Result<NotCurrentContext, GlutError> {
        let config_template = ConfigTemplateBuilder::new()
            .with_buffer_type(glutin::config::ColorBufferType::Rgb {
                r_size: 8,
                g_size: 8,
                b_size: 8,
            })
            .with_api(Api::GLES3)
            .build();
        let config = unsafe { self.display.find_configs(config_template) }?
            .reduce(
                |config, best| match config.num_samples() > best.num_samples() {
                    true => config,
                    false => best,
                },
            )
            .ok_or(GlutError::from(GlutErrorKind::BadDisplay))?;

        let surface_ptr = NonNull::new(surface.id().as_ptr() as *mut c_void)
            .ok_or(GlutError::from(GlutErrorKind::BadDisplay))?;
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr));

        let context_attrs = ContextAttributesBuilder::new().build(Some(raw_window_handle));
        unsafe { self.display.create_context(&config, &context_attrs) }
    }

    pub fn create_surface(
        &self,
        surface: &WlSurface,
        width: NonZero<u32>,
        height: NonZero<u32>,
    ) -> Result<Surface<WindowSurface>, GlutError> {
        let surface_ptr = NonNull::new(surface.id().as_ptr() as *mut c_void)
            .ok_or(GlutError::from(GlutErrorKind::BadDisplay))?;
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr));

        let surface_attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            width,
            height,
        );

        let config_template = ConfigTemplateBuilder::new()
            .with_buffer_type(glutin::config::ColorBufferType::Rgb {
                r_size: 8,
                g_size: 8,
                b_size: 8,
            })
            .with_api(Api::GLES3)
            .build();
        let config = unsafe { self.display.find_configs(config_template) }?
            .reduce(
                |config, best| match config.num_samples() > best.num_samples() {
                    true => config,
                    false => best,
                },
            )
            .ok_or(GlutError::from(GlutErrorKind::BadDisplay))?;
        unsafe { self.display.create_window_surface(&config, &surface_attrs) }
    }
}

pub struct GpuSurface {
    context: PossiblyCurrentContext,
    surface: Surface<WindowSurface>,
    renderer: GLCore,
}

impl GpuSurface {
    pub fn new(
        abstraction: &GlAbstraction,
        surface: &WlSurface,
        width: NonZero<u32>,
        height: NonZero<u32>,
    ) -> Result<GpuSurface, GlutError> {
        let not_context = abstraction.create_context(surface)?;
        let surface = abstraction.create_surface(surface, width, height)?;
        let context = not_context.make_current(&surface)?;

        let renderer = GLCore::new(|fn_name| {
            let c_str = CString::new(fn_name).expect("GL function name invalid C string");
            abstraction.display.get_proc_address(&c_str)
        })
        .map_err(|_| GlutError::from(GlutErrorKind::BadContext))?;

        Ok(GpuSurface {
            context,
            surface,
            renderer,
        })
    }

    pub fn resize(&mut self, width: NonZero<u32>, height: NonZero<u32>) {
        self.surface.resize(&self.context, width, height);
    }

    pub fn swap_buffers(&mut self) -> Result<(), GlutError> {
        self.surface.swap_buffers(&self.context)
    }

    pub fn get_renderer(&self) -> GLCore {
        self.renderer
    }
}
