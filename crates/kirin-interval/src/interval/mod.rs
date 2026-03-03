#[cfg(feature = "arith")]
mod arith_impl;
mod arithmetic;
mod bound;
#[cfg(feature = "cmp")]
mod cmp_impl;
mod domain;
#[cfg(feature = "interpreter")]
mod interpreter_impl;
mod lattice_impl;
#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "interpreter"))]
mod widen_narrow_tests;

pub use arithmetic::{interval_add, interval_mul, interval_neg, interval_sub};
pub use bound::Bound;
pub use domain::Interval;
