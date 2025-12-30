use std::ffi::c_void;
use std::path::Path;

use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_3_0_g, GLCore, GLCoreError};

use crate::opengl::shaders::builtin::{BuiltinShader, NoShader};
use crate::opengl::shaders::{MatrixShader, NoMatrixShader, UninitShaderProgram};
use crate::opengl::types::{AsFloatArray, Indices, IndicesBackend, Vec2, Vec2Array};

use super::types::GlResult;
use super::{
    shaders::ColorShader,
    shaders::{ShaderBundle, ShaderProgram},
};

#[derive(Debug, Clone, Copy)]
pub enum ElementsMode {
    Points,
    LineStrip,
    LineLoop,
    Lines,
    LineStripAdjacency,
    LinesAdjacency,
    TriangleStrip,
    TriangleFan,
    Triangles,
    TriangleStripAdjacency,
    TrianglesAdjacency,
}

impl ElementsMode {
    pub fn into_opengl_mode(self) -> u32 {
        match self {
            ElementsMode::Points => glcore::GL_POINTS,
            ElementsMode::LineStrip => glcore::GL_LINE_STRIP,
            ElementsMode::LineLoop => glcore::GL_LINE_LOOP,
            ElementsMode::Lines => glcore::GL_LINES,
            ElementsMode::LineStripAdjacency => glcore::GL_LINE_STRIP_ADJACENCY,
            ElementsMode::LinesAdjacency => glcore::GL_LINES_ADJACENCY,
            ElementsMode::TriangleStrip => glcore::GL_TRIANGLE_STRIP,
            ElementsMode::TriangleFan => glcore::GL_TRIANGLE_FAN,
            ElementsMode::Triangles => glcore::GL_TRIANGLES,
            ElementsMode::TriangleStripAdjacency => glcore::GL_TRIANGLE_STRIP_ADJACENCY,
            ElementsMode::TrianglesAdjacency => glcore::GL_TRIANGLES_ADJACENCY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimpleGL<State> {
    core: GLCore,
    current_shader: Option<ShaderProgram<State>>,
}

impl SimpleGL<NoShader> {
    pub fn new(core: GLCore) -> SimpleGL<NoShader> {
        SimpleGL {
            core,
            current_shader: None,
        }
    }
}

impl<S> SimpleGL<S> {
    pub fn new_shader_program(
        &self,
        vertex: String,
        fragment: String,
    ) -> GlResult<UninitShaderProgram<S>> {
        ShaderBundle::new_from_sources(self.core, vertex, fragment)?.link()
    }

    pub fn new_builtin_shader<T: BuiltinShader<Properties = T>>(
        &self,
        builtin: T,
    ) -> GlResult<UninitShaderProgram<T>> {
        builtin.into_program(self.core)
    }

    pub fn new_shader_program_from_files<P0: AsRef<Path>, P1: AsRef<Path>>(
        &self,
        vertex: P0,
        fragment: P1,
    ) -> GlResult<UninitShaderProgram<S>> {
        ShaderBundle::new_from_files(self.core, vertex, fragment)?.link()
    }

    pub fn clear(&self, r: f32, g: f32, b: f32, a: f32) -> GlResult<()> {
        self.core.glClearColor(r, g, b, a)?;
        self.core
            .glClear(glcore::GL_COLOR_BUFFER_BIT | glcore::GL_DEPTH_BUFFER_BIT)
    }

    pub fn with_shader<N>(self, shader: ShaderProgram<N>) -> SimpleGL<N> {
        SimpleGL {
            core: self.core,
            current_shader: Some(shader),
        }
    }
}

impl<S: ColorShader + MatrixShader> SimpleGL<S> {
    pub fn draw_rectangle(&self, pos: Vec2, size: Vec2) -> GlResult<()> {
        self.current_shader
            .as_ref()
            .ok_or(GLCoreError::InvalidOperation("No shader loaded"))?
            .set_matrix(pos, size)?;

        let vertices_backend = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
        ];
        let indices_backend = [0, 1, 2, 3];
        let indices = Indices::<u32>::new(&indices_backend);
        let vertices = Vec2Array::new(&vertices_backend);

        self.draw_polygon_indices(ElementsMode::TriangleStrip, vertices, indices)
    }
}

impl<S: ColorShader + NoMatrixShader> SimpleGL<S> {
    pub fn draw_rectangle_generic(&self, topleft: Vec2, size: Vec2) -> GlResult<()> {
        let vertices = [
            topleft,
            topleft + size * Vec2::new(1.0, 0.0),
            topleft + size * Vec2::new(0.0, 1.0),
            topleft + size,
        ];
        let indices = [0, 1, 2, 3];
        self.draw_polygon_indices(
            ElementsMode::TriangleStrip,
            Vec2Array::new(&vertices),
            Indices::<u32>::new(&indices),
        )
    }
}

impl<S: ColorShader> SimpleGL<S> {
    pub fn draw_polygon<V>(&self, mode: ElementsMode, vertices: V) -> GlResult<()>
    where
        V: AsFloatArray<Backend = Vec2>,
    {
        let vert_ref = vertices
            .as_contiguous_block()
            .ok_or(GLCoreError::InvalidValue(
                "Polygon vector cannot be zero sized",
            ))?;

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
            std::mem::size_of_val(vert_ref),
            vert_ref.as_ptr() as *const c_void,
            glcore::GL_STATIC_DRAW,
        )?;

        // Assign attribute to attribute buffer
        self.core.glEnableVertexAttribArray(0)?;
        self.core.glVertexAttribPointer(
            0,
            V::FLOATS_PER_ELEMENT as i32,
            glcore::GL_FLOAT,
            glcore::GL_FALSE as u8,
            0,
            std::ptr::null(),
        )?;

        self.core.glDrawArrays(
            mode.into_opengl_mode(),
            0,
            (vert_ref.len() / V::FLOATS_PER_ELEMENT) as i32,
        )?;
        self.core.glDisableVertexAttribArray(0)?;
        self.core.glDeleteBuffers(1, [vertex_buffer].as_ptr())?;
        Ok(())
    }

    pub fn draw_polygon_indices<V, B>(
        &self,
        mode: ElementsMode,
        vertices: V,
        indices: Indices<B>,
    ) -> GlResult<()>
    where
        V: AsFloatArray<Backend = Vec2>,
        B: IndicesBackend,
    {
        let vert_ref = vertices
            .as_contiguous_block()
            .ok_or(GLCoreError::InvalidValue(
                "Polygon vector cannot be zero sized",
            ))?;

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
            std::mem::size_of_val(vert_ref),
            vert_ref.as_ptr() as *const c_void,
            glcore::GL_STATIC_DRAW,
        )?;

        // Same as data buffer, but for indices
        let mut index_buffer = 0;
        self.core.glGenBuffers(1, &mut index_buffer)?;
        self.core
            .glBindBuffer(glcore::GL_ELEMENT_ARRAY_BUFFER, index_buffer)?;
        self.core.glBufferData(
            glcore::GL_ELEMENT_ARRAY_BUFFER,
            indices.len() * std::mem::size_of::<B::Backend>(),
            indices.ptr(),
            glcore::GL_STATIC_DRAW,
        )?;

        // Assign attribute to attribute buffer
        self.core.glEnableVertexAttribArray(0)?;
        self.core.glVertexAttribPointer(
            0,
            V::FLOATS_PER_ELEMENT as i32,
            glcore::GL_FLOAT,
            glcore::GL_FALSE as u8,
            0,
            std::ptr::null(),
        )?;

        self.core.glDrawElements(
            mode.into_opengl_mode(),
            indices.len() as i32,
            B::get_opengl_type(),
            std::ptr::null(),
        )?;
        self.core.glDisableVertexAttribArray(0)?;
        self.core
            .glDeleteBuffers(2, [vertex_buffer, index_buffer].as_ptr())?;
        Ok(())
    }
}
