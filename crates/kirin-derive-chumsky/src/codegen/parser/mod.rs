//! Code generation for the `HasDialectParser` derive macro.

mod chain;
mod dialect_emit_ir;
mod generate;
mod impl_gen;
mod parser_emit_ir;

pub use generate::GenerateHasDialectParser;
