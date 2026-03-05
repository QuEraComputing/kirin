mod context;
mod emit;
mod helpers;
mod scan;
mod statement;

/// Generates field iterator trait implementations.
///
/// Produces `HasArguments`, `HasResults`, `HasBlocks`, `HasSuccessors`, and
/// `HasRegions` impls with both immutable and mutable iterators.
pub use context::DeriveFieldIter;

/// Which field category to generate iterators for.
pub use context::FieldIterKind;
