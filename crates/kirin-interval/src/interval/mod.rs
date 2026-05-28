#[cfg(feature = "arith")]
mod arith_impl;
#[cfg(all(test, feature = "arith"))]
mod arith_tests;
mod arithmetic;
#[cfg(feature = "bitwise")]
mod bitwise_impl;
mod bound;
#[cfg(feature = "cmp")]
mod cmp_impl;
#[cfg(all(test, feature = "cmp"))]
mod cmp_tests;
mod domain;
mod lattice_impl;
#[cfg(test)]
mod tests;

pub use arithmetic::{
    interval_add, interval_div, interval_mul, interval_neg, interval_rem, interval_sub,
};
pub use bound::Bound;
pub use domain::Interval;
