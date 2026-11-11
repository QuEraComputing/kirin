//! Library sorts and static constructor sets, for dialects to compose into
//! their own.

mod boolean;
mod int;

pub use boolean::BoolStaticCon;
pub use int::{IntSort, IntSortT, IntStaticCon};
