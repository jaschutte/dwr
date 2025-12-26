use glcore::{GL_1_0_g, GL_1_1_g, GL_1_5_g, GL_2_0_g, GL_3_0_g};
use glcore::{GLCore, GLCoreError};
use std::ffi::{CStr, CString};
use std::path::Path;

type GlResult<T> = Result<T, GLCoreError>;

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
    graphics: &GLCore,
    shader_or_program: u32,
    validate_type: ProgramValidation,
) -> GlResult<()> {
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
        if cfg!(debug_assertions) {
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
    graphics: GLCore,
}

impl Shader {
    pub fn load_shader_from_file<P: AsRef<Path>>(
        graphics: GLCore,
        kind: ShaderKind,
        path: P,
    ) -> GlResult<Shader> {
        let source = std::fs::read_to_string(path)
            .map_err(|_| GLCoreError::InvalidValue("Invalid shader file path"))?;
        Self::load_shader(graphics, kind, source)
    }

    pub fn load_shader(graphics: GLCore, kind: ShaderKind, mut source: String) -> GlResult<Shader> {
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
        let shader_id = graphics.glCreateShader(kind.kind())?;

        graphics.glShaderSource(shader_id, 1, shader_sources.as_ptr(), std::ptr::null())?;
        graphics.glCompileShader(shader_id)?;
        validate_shader_step(&graphics, shader_id, kind.into())?;

        Ok(Shader {
            shader_id,
            kind,
            graphics,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShaderBundle {
    vertex: Shader,
    fragment: Shader,
    graphics: GLCore,
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
        if vertex.graphics != fragment.graphics {
            return Err(GLCoreError::InvalidValue(
                "Vertex and fragment shaders have differring OpenGL APIs (glcore::GLCore)",
            ));
        }
        Ok(ShaderBundle {
            vertex,
            fragment,
            graphics: vertex.graphics,
        })
    }

    pub fn new_from_sources(
        graphics: GLCore,
        vertex: String,
        fragment: String,
    ) -> GlResult<ShaderBundle> {
        Ok(ShaderBundle {
            vertex: Shader::load_shader(graphics, ShaderKind::Vertex, vertex)?,
            fragment: Shader::load_shader(graphics, ShaderKind::Fragment, fragment)?,
            graphics,
        })
    }

    pub fn new_from_files<P0: AsRef<Path>, P1: AsRef<Path>>(
        graphics: GLCore,
        vertex: P0,
        fragment: P1,
    ) -> GlResult<ShaderBundle> {
        Ok(ShaderBundle {
            vertex: Shader::load_shader_from_file(graphics, ShaderKind::Vertex, vertex)?,
            fragment: Shader::load_shader_from_file(graphics, ShaderKind::Fragment, fragment)?,
            graphics,
        })
    }

    pub fn link(self) -> GlResult<ShaderProgram> {
        let shader_program = self.graphics.glCreateProgram()?;
        self.graphics
            .glAttachShader(shader_program, self.vertex.shader_id)?;
        self.graphics
            .glAttachShader(shader_program, self.fragment.shader_id)?;
        self.graphics.glLinkProgram(shader_program)?;
        validate_shader_step(&self.graphics, shader_program, ProgramValidation::Linking)?;

        self.graphics
            .glDetachShader(shader_program, self.vertex.shader_id)?;
        self.graphics
            .glDetachShader(shader_program, self.fragment.shader_id)?;
        self.graphics.glDeleteShader(self.vertex.shader_id)?;
        self.graphics.glDeleteShader(self.fragment.shader_id)?;

        Ok(ShaderProgram {
            program: shader_program,
            graphics: self.graphics,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderProgram {
    program: u32,
    graphics: GLCore,
}

impl ShaderProgram {
    pub fn use_program(&self) -> GlResult<()> {
        self.graphics.glUseProgram(self.program)
    }

    pub fn locate_uniform(&self, variable: &CStr) -> GlResult<i32> {
        self.graphics.glGetUniformLocation(self.program, variable.as_ptr())
    }
}
