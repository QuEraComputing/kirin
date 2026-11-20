//! Cloning statements and CFGs for the [`Rewriter`](super::Rewriter).

mod deep_copy;
mod fill;
mod map;
mod reserve;

pub use map::IRMap;
