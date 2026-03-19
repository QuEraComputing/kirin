mod bool_interval;
mod interval;

pub use bool_interval::BoolInterval;
pub use interval::{Bound, Interval, interval_add, interval_mul, interval_neg, interval_sub};
