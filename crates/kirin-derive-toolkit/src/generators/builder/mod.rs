mod context;
mod emit;
mod helpers;
mod scan;
mod statement;

/// Generates constructor functions for IR statements.
///
/// Emits `new(...)` methods on structs, or per-variant constructors for enums,
/// based on the statement's fields and `#[kirin(builder = ...)]` options.
pub use context::DeriveBuilder;
