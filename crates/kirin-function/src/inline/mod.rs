//! Inlining calls to specialized functions.

mod calls;
mod error;
mod ops;
#[cfg(test)]
mod tests;

pub use calls::inline_calls;
pub use error::InlineError;
pub use ops::FunctionOps;
