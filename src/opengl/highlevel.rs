use std::ffi::c_void;
use std::marker::PhantomData;
use std::path::Path;

use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_3_0_g, GLCore, GLCoreError};

use crate::opengl::types::{AsFloatArray, Vec3, Vec3Array};

use super::types::GlResult;
use super::{
    shaders,
    shaders::{ShaderBundle, ShaderProgram},
};

pub struct SimpleState;
pub trait InSimpleState {}
impl InSimpleState for SimpleState {}

pub struct ShadedState;
pub trait InShadedState {}
impl InShadedState for ShadedState {}

#[derive(Debug, Clone)]
pub struct SimpleGL<State> {
    core: GLCore,
    _phantom: PhantomData<State>,
}

impl<S0> SimpleGL<S0> {
    fn coerce_state<S1>(self) -> SimpleGL<S1> {
        SimpleGL {
            core: self.core,
            _phantom: PhantomData,
        }
    }
}

impl<State> SimpleGL<State> {
    pub fn new(core: GLCore) -> SimpleGL<SimpleState> {
        SimpleGL {
            core,
            _phantom: PhantomData,
        }
    }

    pub fn new_shader_program(
        &self,
        vertex: String,
        fragment: String,
    ) -> GlResult<ShaderProgram<shaders::IdleState>> {
        ShaderBundle::new_from_sources(self.core, vertex, fragment)?.link()
    }

    pub fn new_shader_program_from_files<P0: AsRef<Path>, P1: AsRef<Path>>(
        &self,
        vertex: P0,
        fragment: P1,
    ) -> GlResult<ShaderProgram<shaders::IdleState>> {
        ShaderBundle::new_from_files(self.core, vertex, fragment)?.link()
    }

    pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) -> GlResult<()> {
        self.core.glClearColor(r, g, b, a)?;
        self.core.glClear(glcore::GL_COLOR_BUFFER_BIT | glcore::GL_DEPTH_BUFFER_BIT)
    }
}

impl<S: InSimpleState> SimpleGL<S> {
    pub fn shaded(self, shader: &ShaderProgram<shaders::ActiveState>) -> SimpleGL<ShadedState> {
        let _ = shader;
        self.coerce_state()
    }
}

impl<S: InShadedState> SimpleGL<S> {
    pub fn draw_polygon<V>(&self, vertices: V) -> GlResult<()>
    where
        V: AsFloatArray<Backend = Vec3>,
    {
        // Create the attribute buffer
        let mut vertex_attributes = 0;
        self.core.glGenVertexArrays(1, &mut vertex_attributes)?;
        self.core.glBindVertexArray(vertex_attributes)?;

        // Create & copy over data to the data buffer
        let mut vertex_buffer = 0;
        self.core.glGenBuffers(1, &mut vertex_buffer)?;
        self.core
            .glBindBuffer(glcore::GL_ARRAY_BUFFER, vertex_buffer)?;
        self.core.glBufferData(
            glcore::GL_ARRAY_BUFFER,
            std::mem::size_of::<[f32; 9]>(),
            vertices
                .as_contiguous_block()
                .ok_or(GLCoreError::InvalidValue(
                    "Polygon vector cannot be zero sized",
                ))?
                .as_ptr() as *const c_void,
            glcore::GL_STATIC_DRAW,
        )?;

        // Assign attribute to attribute buffer
        self.core.glEnableVertexAttribArray(0)?;
        self.core.glVertexAttribPointer(
            0,
            3,
            glcore::GL_FLOAT,
            glcore::GL_FALSE as u8,
            0,
            std::ptr::null(),
        )?;

        self.core.glDrawArrays(glcore::GL_TRIANGLES, 0, 3)?;
        self.core.glDisableVertexAttribArray(0)?;
        Ok(())
    }
}
