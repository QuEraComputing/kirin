mod display;
mod error;
mod sorts;
mod statics;

pub use error::SortError;
pub use sorts::{BoolStaticCon, FnStaticCon, IntSort, IntSortT, IntStaticCon, UnitStaticCon};
pub use statics::{
    BaseSort, IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId,
    StaticVar, StaticVarId, Statics, TypeSig,
};
