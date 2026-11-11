mod display;
mod error;
mod statics;

pub use error::SortError;
pub use statics::{
    IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId, StaticVar,
    StaticVarId, Statics,
};
