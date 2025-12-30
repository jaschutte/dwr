use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_2_1_g, GL_3_0_g};
use glcore::{GLCore, GLCoreError};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::path::Path;

use crate::opengl::types::{Vec2, Vec4};

use super::types::GlResult;

pub mod builtin {
    use super::ShaderBundle;
    use crate::opengl::{shaders::UninitShaderProgram, types::GlResult};
    use glcore::GLCore;

    macro_rules! builtin_shader {
        ($name:ident <- $file:literal | $($properties:ident):*) => {
            builtin_shader!($name <- $file);

            $(
                impl super::$properties for $name {}
            )*
        };
        ($name:ident <- $file:literal) => {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub struct $name;

            impl BuiltinShader for $name {
                type Properties = $name;

                fn get_vertex_static(self) -> &'static str {
                    include_str!(concat!("shaders/", $file, ".vert"))
                }
                fn get_fragment_static(self) -> &'static str {
                    include_str!(concat!("shaders/", $file, ".frag"))
                }
                fn get_vertex(self) -> String {
                    self.get_vertex_static().to_string()
                }
                fn get_fragment(self) -> String {
                    self.get_fragment_static().to_string()
                }
                fn into_program(self, core: GLCore) -> GlResult<UninitShaderProgram<Self::Properties>> {
                    ShaderBundle::new_from_sources(core, self.get_vertex(), self.get_fragment())?.link()
                }
            }
        };
    }

    pub trait BuiltinShader {
        type Properties;

        fn get_vertex_static(self) -> &'static str;
        fn get_fragment_static(self) -> &'static str;
        fn get_vertex(self) -> String;
        fn get_fragment(self) -> String;
        fn into_program(self, core: GLCore) -> GlResult<UninitShaderProgram<Self::Properties>>;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct NoShader;

    impl BuiltinShader for NoShader {
        type Properties = NoShader;

        fn get_vertex_static(self) -> &'static str {
            ""
        }

        fn get_fragment_static(self) -> &'static str {
            ""
        }

        fn get_vertex(self) -> String {
            String::new()
        }

        fn get_fragment(self) -> String {
            String::new()
        }

        fn into_program(self, _: GLCore) -> GlResult<UninitShaderProgram<Self::Properties>> {
            Err(glcore::GLCoreError::InvalidOperation(
                "Cannot create a shader program for the NoShader builtin",
            ))
        }
    }

    builtin_shader!(FlatColor <- "flat_color" | ColorShader:NoMatrixShader);
    builtin_shader!(QuadColor <- "quad_color" | ColorShader:MatrixShader);
}

#[derive(Debug, Clone, Copy)]
pub enum ProgramValidation {
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

fn validate_shader_step(
    core: &GLCore,
    shader_or_program: u32,
    validate_type: ProgramValidation,
) -> GlResult<()> {
    let mut shader_status = 0;
    let mut shader_status_len = 0;
    let pname = validate_type.pname();

    match validate_type.is_program() {
        true => {
            core.glGetProgramiv(shader_or_program, pname, &mut shader_status)?;
            core.glGetProgramiv(
                shader_or_program,
                glcore::GL_INFO_LOG_LENGTH,
                &mut shader_status_len,
            )?;
        }
        false => {
            core.glGetShaderiv(shader_or_program, pname, &mut shader_status)?;
            core.glGetShaderiv(
                shader_or_program,
                glcore::GL_INFO_LOG_LENGTH,
                &mut shader_status_len,
            )?;
        }
    }
    if shader_status_len > 0 {
        if cfg!(debug_assertions) {
            println!("Failed {} ({shader_status}):", validate_type.label());
            let mut log: [glcore::GLchar; 512] = [0; 512];
            match validate_type.is_program() {
                true => {
                    core.glGetProgramInfoLog(
                        shader_or_program,
                        512,
                        std::ptr::null_mut(),
                        log.as_mut_ptr(),
                    )?;
                }
                false => {
                    core.glGetShaderInfoLog(
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
        };
        Err(glcore::GLCoreError::UnknownError((
            1,
            "Shader failed compilation",
        )))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderKind {
    Vertex,
    Fragment,
}

impl ShaderKind {
    pub fn kind(self) -> u32 {
        match self {
            ShaderKind::Vertex => glcore::GL_VERTEX_SHADER,
            ShaderKind::Fragment => glcore::GL_FRAGMENT_SHADER,
        }
    }
}

impl From<ShaderKind> for ProgramValidation {
    fn from(value: ShaderKind) -> Self {
        match value {
            ShaderKind::Vertex => ProgramValidation::Vertex,
            ShaderKind::Fragment => ProgramValidation::Fragment,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shader {
    shader_id: u32,
    kind: ShaderKind,
    core: GLCore,
}

impl Shader {
    pub fn load_shader_from_file<P: AsRef<Path>>(
        core: GLCore,
        kind: ShaderKind,
        path: P,
    ) -> GlResult<Shader> {
        let source = std::fs::read_to_string(path)
            .map_err(|_| GLCoreError::InvalidValue("Invalid shader file path"))?;
        Self::load_shader(core, kind, source)
    }

    pub fn load_shader(core: GLCore, kind: ShaderKind, mut source: String) -> GlResult<Shader> {
        if !source.is_ascii() {
            return Err(GLCoreError::InvalidValue(
                "Shader source must only contain ASCII",
            ));
        }

        if !matches!(source.as_bytes().last(), Some(b'\0')) {
            source.push('\0');
        }
        let cstr_source = CStr::from_bytes_with_nul(source.as_bytes()).map_err(|_| {
            GLCoreError::InvalidValue("Shader source cannot be represented as a C-style string")
        })?;
        let shader_sources = [cstr_source.as_ptr()];
        let shader_id = core.glCreateShader(kind.kind())?;

        core.glShaderSource(shader_id, 1, shader_sources.as_ptr(), std::ptr::null())?;
        core.glCompileShader(shader_id)?;
        validate_shader_step(&core, shader_id, kind.into())?;

        Ok(Shader {
            shader_id,
            kind,
            core,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShaderBundle {
    vertex: Shader,
    fragment: Shader,
    core: GLCore,
}

impl ShaderBundle {
    pub fn new(vertex: Shader, fragment: Shader) -> GlResult<ShaderBundle> {
        if !matches!(vertex.kind, ShaderKind::Vertex) {
            return Err(GLCoreError::InvalidValue(
                "Passed vertex shader is not a vertex shader",
            ));
        }
        if !matches!(fragment.kind, ShaderKind::Fragment) {
            return Err(GLCoreError::InvalidValue(
                "Passed fragment shader is not a fragment shader",
            ));
        }
        if vertex.core != fragment.core {
            return Err(GLCoreError::InvalidValue(
                "Vertex and fragment shaders have differring OpenGL APIs (glcore::GLCore)",
            ));
        }
        Ok(ShaderBundle {
            vertex,
            fragment,
            core: vertex.core,
        })
    }

    pub fn new_from_sources(
        core: GLCore,
        vertex: String,
        fragment: String,
    ) -> GlResult<ShaderBundle> {
        Ok(ShaderBundle {
            vertex: Shader::load_shader(core, ShaderKind::Vertex, vertex)?,
            fragment: Shader::load_shader(core, ShaderKind::Fragment, fragment)?,
            core,
        })
    }

    pub fn new_from_files<P0: AsRef<Path>, P1: AsRef<Path>>(
        core: GLCore,
        vertex: P0,
        fragment: P1,
    ) -> GlResult<ShaderBundle> {
        Ok(ShaderBundle {
            vertex: Shader::load_shader_from_file(core, ShaderKind::Vertex, vertex)?,
            fragment: Shader::load_shader_from_file(core, ShaderKind::Fragment, fragment)?,
            core,
        })
    }

    pub fn link<F>(self) -> GlResult<UninitShaderProgram<F>> {
        let shader_program = self.core.glCreateProgram()?;
        self.core
            .glAttachShader(shader_program, self.vertex.shader_id)?;
        self.core
            .glAttachShader(shader_program, self.fragment.shader_id)?;
        self.core.glLinkProgram(shader_program)?;
        validate_shader_step(&self.core, shader_program, ProgramValidation::Linking)?;

        self.core
            .glDetachShader(shader_program, self.vertex.shader_id)?;
        self.core
            .glDetachShader(shader_program, self.fragment.shader_id)?;
        self.core.glDeleteShader(self.vertex.shader_id)?;
        self.core.glDeleteShader(self.fragment.shader_id)?;

        Ok(UninitShaderProgram {
            program: shader_program,
            core: self.core,
            _phantom: PhantomData,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UniformKind<'a> {
    Uniform1f(f32),
    Uniform2f(f32, f32),
    Uniform3f(f32, f32, f32),
    Uniform4f(f32, f32, f32, f32),
    Uniform1i(i32),
    Uniform2i(i32, i32),
    Uniform3i(i32, i32, i32),
    Uniform4i(i32, i32, i32, i32),
    Uniform1ui(u32),
    Uniform2ui(u32, u32),
    Uniform3ui(u32, u32, u32),
    Uniform4ui(u32, u32, u32, u32),
    Uniform1fv(i32, &'a [f32]),
    Uniform2fv(i32, &'a [f32]),
    Uniform3fv(i32, &'a [f32]),
    Uniform4fv(i32, &'a [f32]),
    Uniform1iv(i32, &'a [i32]),
    Uniform2iv(i32, &'a [i32]),
    Uniform3iv(i32, &'a [i32]),
    Uniform4iv(i32, &'a [i32]),
    Uniform1uiv(i32, &'a [u32]),
    Uniform2uiv(i32, &'a [u32]),
    Uniform3uiv(i32, &'a [u32]),
    Uniform4uiv(i32, &'a [u32]),
    UniformMatrix2fv(i32, bool, &'a [f32]),
    UniformMatrix3fv(i32, bool, &'a [f32]),
    UniformMatrix4fv(i32, bool, &'a [f32]),
    UniformMatrix2x3fv(i32, bool, &'a [f32]),
    UniformMatrix3x2fv(i32, bool, &'a [f32]),
    UniformMatrix2x4fv(i32, bool, &'a [f32]),
    UniformMatrix4x2fv(i32, bool, &'a [f32]),
    UniformMatrix3x4fv(i32, bool, &'a [f32]),
    UniformMatrix4x3fv(i32, bool, &'a [f32]),
}

impl<'a> UniformKind<'a> {
    fn exec(self, core: &GLCore, location: i32) -> GlResult<()> {
        match self {
            Self::Uniform1f(v0) => core.glUniform1f(location, v0)?,
            Self::Uniform2f(v0, v1) => core.glUniform2f(location, v0, v1)?,
            Self::Uniform3f(v0, v1, v2) => core.glUniform3f(location, v0, v1, v2)?,
            Self::Uniform4f(v0, v1, v2, v3) => core.glUniform4f(location, v0, v1, v2, v3)?,
            Self::Uniform1i(v0) => core.glUniform1i(location, v0)?,
            Self::Uniform2i(v0, v1) => core.glUniform2i(location, v0, v1)?,
            Self::Uniform3i(v0, v1, v2) => core.glUniform3i(location, v0, v1, v2)?,
            Self::Uniform4i(v0, v1, v2, v3) => core.glUniform4i(location, v0, v1, v2, v3)?,
            Self::Uniform1ui(v0) => core.glUniform1ui(location, v0)?,
            Self::Uniform2ui(v0, v1) => core.glUniform2ui(location, v0, v1)?,
            Self::Uniform3ui(v0, v1, v2) => core.glUniform3ui(location, v0, v1, v2)?,
            Self::Uniform4ui(v0, v1, v2, v3) => core.glUniform4ui(location, v0, v1, v2, v3)?,
            Self::Uniform1fv(count, value) => core.glUniform1fv(location, count, value.as_ptr())?,
            Self::Uniform2fv(count, value) => core.glUniform2fv(location, count, value.as_ptr())?,
            Self::Uniform3fv(count, value) => core.glUniform3fv(location, count, value.as_ptr())?,
            Self::Uniform4fv(count, value) => core.glUniform4fv(location, count, value.as_ptr())?,
            Self::Uniform1iv(count, value) => core.glUniform1iv(location, count, value.as_ptr())?,
            Self::Uniform2iv(count, value) => core.glUniform2iv(location, count, value.as_ptr())?,
            Self::Uniform3iv(count, value) => core.glUniform3iv(location, count, value.as_ptr())?,
            Self::Uniform4iv(count, value) => core.glUniform4iv(location, count, value.as_ptr())?,
            Self::Uniform1uiv(count, value) => {
                core.glUniform1uiv(location, count, value.as_ptr())?
            }
            Self::Uniform2uiv(count, value) => {
                core.glUniform2uiv(location, count, value.as_ptr())?
            }
            Self::Uniform3uiv(count, value) => {
                core.glUniform3uiv(location, count, value.as_ptr())?
            }
            Self::Uniform4uiv(count, value) => {
                core.glUniform4uiv(location, count, value.as_ptr())?
            }
            Self::UniformMatrix2fv(count, transpose, value) => {
                core.glUniformMatrix2fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix3fv(count, transpose, value) => {
                core.glUniformMatrix3fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix4fv(count, transpose, value) => {
                core.glUniformMatrix4fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix2x3fv(count, transpose, value) => {
                core.glUniformMatrix2x3fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix3x2fv(count, transpose, value) => {
                core.glUniformMatrix3x2fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix2x4fv(count, transpose, value) => {
                core.glUniformMatrix2x4fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix4x2fv(count, transpose, value) => {
                core.glUniformMatrix4x2fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix3x4fv(count, transpose, value) => {
                core.glUniformMatrix3x4fv(location, count, transpose as u8, value.as_ptr())?
            }
            Self::UniformMatrix4x3fv(count, transpose, value) => {
                core.glUniformMatrix4x3fv(location, count, transpose as u8, value.as_ptr())?
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UninitShaderProgram<F> {
    program: u32,
    core: GLCore,
    _phantom: PhantomData<F>,
}

impl<F> UninitShaderProgram<F> {
    pub fn use_program(self) -> GlResult<ShaderProgram<F>> {
        self.core.glUseProgram(self.program)?;
        Ok(ShaderProgram {
            program: self.program,
            core: self.core,
            _phantom: PhantomData,
        })
    }
}

pub trait ColorShader {}
pub trait MatrixShader {}
pub trait NoMatrixShader {}

#[derive(Debug, Clone, Copy)]
pub struct ShaderProgram<F> {
    program: u32,
    core: GLCore,
    _phantom: PhantomData<F>,
}

impl<F> ShaderProgram<F> {
    pub fn set_uniform(&self, variable: &CStr, uniform: UniformKind) -> GlResult<()> {
        let location = self
            .core
            .glGetUniformLocation(self.program, variable.as_ptr())?;
        uniform.exec(&self.core, location)
    }
}

impl<F: ColorShader> ShaderProgram<F> {
    pub fn set_color(&self, color: Vec4) -> GlResult<()> {
        self.set_uniform(
            c"color",
            UniformKind::Uniform4f(color.x, color.y, color.z, color.w),
        )
    }

    pub fn set_color_rgba(&self, r: f32, g: f32, b: f32, a: f32) -> GlResult<()> {
        self.set_uniform(c"color", UniformKind::Uniform4f(r, g, b, a))
    }
}

impl<F: ColorShader> ShaderProgram<F> {
    pub fn set_matrix(&self, pos: Vec2, size: Vec2) -> GlResult<()> {
        self.set_uniform(
            c"matrix",
            UniformKind::Uniform4f(pos.x, pos.y, size.x, size.y),
        )
    }
}
