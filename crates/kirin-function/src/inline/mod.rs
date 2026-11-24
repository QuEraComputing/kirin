//! Inlining calls to specialized functions.

mod calls;
mod check;
mod error;
mod not_inlined;
mod ops;
#[cfg(test)]
mod tests;

pub use calls::{InlineReport, inline_calls};
pub use error::InlineError;
pub use not_inlined::NotInlined;
pub use ops::FunctionOps;
