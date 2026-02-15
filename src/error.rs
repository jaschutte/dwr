use std::fmt::Debug;

#[derive(Debug, Clone)]
pub enum Error {
    OpenGL(glcore::GLCoreError),
    Glutin(glutin::error::Error),
}

impl From<glcore::GLCoreError> for Error {
    fn from(value: glcore::GLCoreError) -> Self {
        Error::OpenGL(value)
    }
}

impl From<glutin::error::Error> for Error {
    fn from(value: glutin::error::Error) -> Self {
        Error::Glutin(value)
    }
}

// impl<T> From<Result<T, glcore::GLCoreError>> for Result<T, Error> {
//     fn from(value: Result<T, glcore::GLCoreError>) -> Self {
//         value.map_err(|err| err.into())
//     }
// }
//
// impl<T> From<Result<T, glutin::error::Error>> for Result<T, Error> {
//     fn from(value: Result<T, glutin::error::Error>) -> Self {
//         value.map_err(|err| err.into())
//     }
// }

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::OpenGL(glcore_error) => glcore_error.fmt(f),
            Error::Glutin(error) => std::fmt::Display::fmt(error, f),
        };
        Ok(())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::OpenGL(glcore_error) => None,
            Error::Glutin(error) => Some(error),
        }
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}
