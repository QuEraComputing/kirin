use thiserror::Error;

/// Errors produced while lowering the Python AST subset to Kirin IR.
#[derive(Debug, Error)]
pub enum LowerError {
    #[error("undefined name: {0}")]
    UndefinedName(String),
    #[error("unsupported Python construct: {0}")]
    Unsupported(String),
    #[error("unknown function called: {0}")]
    UnknownFunction(String),
    #[error("IR builder error: {0}")]
    Builder(String),
}
