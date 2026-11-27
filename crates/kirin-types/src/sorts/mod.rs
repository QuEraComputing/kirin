//! Library sorts and static constructor sets, for dialects to compose into
//! their own.

mod boolean;
mod function;
mod int;
mod unit;

pub use boolean::BoolStaticCon;
pub use function::FnStaticCon;
pub use int::{IntSort, IntSortT, IntStaticCon};
pub use unit::UnitStaticCon;
