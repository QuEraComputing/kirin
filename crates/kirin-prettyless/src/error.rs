//! Error types for pretty printing and rendering.

use std::fmt;
use std::io;

use kirin_ir::Function;

/// Errors that can occur during rendering.
#[derive(Debug)]
pub enum RenderError {
    /// An I/O error occurred while writing rendered output.
    Io(io::Error),
    /// A formatting error occurred during document rendering.
    Fmt(fmt::Error),
    /// The requested function ID was not found in the pipeline.
    UnknownFunction(Function),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Io(err) => write!(f, "I/O error during rendering: {err}"),
            RenderError::Fmt(err) => write!(f, "formatting error during rendering: {err}"),
            RenderError::UnknownFunction(func) => {
                write!(f, "function {func:?} not found in pipeline")
            }
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RenderError::Io(err) => Some(err),
            RenderError::Fmt(err) => Some(err),
            RenderError::UnknownFunction(_) => None,
        }
    }
}

impl From<io::Error> for RenderError {
    fn from(err: io::Error) -> Self {
        RenderError::Io(err)
    }
}

impl From<fmt::Error> for RenderError {
    fn from(err: fmt::Error) -> Self {
        RenderError::Fmt(err)
    }
}
