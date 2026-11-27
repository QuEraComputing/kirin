mod display;
mod error;
mod sorts;
mod statics;

pub use error::SortError;
pub use sorts::{BoolStaticCon, IntSort, IntSortT, IntStaticCon};
pub use statics::{
    BaseSort, IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId,
    StaticVar, StaticVarId, Statics,
};
