//! `HasParser` implementations for common builtin types.
//!
//! This module provides parser support for:
//! - Signed integers: `i8`, `i16`, `i32`, `i64`, `isize`
//! - Unsigned integers: `u8`, `u16`, `u32`, `u64`, `usize`
//! - Floating point: `f32`, `f64`
//! - Boolean: `bool`
//! - String: `String`
//!
//! Note: `PrettyPrint` implementations for these types are in `kirin-prettyless`
//! due to the orphan rule.

mod float;
mod integer;
mod primitive;
mod signature;

#[cfg(test)]
mod tests;
