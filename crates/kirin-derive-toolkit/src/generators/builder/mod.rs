mod context;
mod emit;
pub(crate) mod helpers;
mod scan;
pub(crate) mod statement;

/// Generates constructor functions for IR statements.
///
/// Emits `new(...)` methods on structs, or per-variant constructors for enums,
/// based on the statement's fields and `#[kirin(builder = ...)]` options.
pub use context::DeriveBuilder;
