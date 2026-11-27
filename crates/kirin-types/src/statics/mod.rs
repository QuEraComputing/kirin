//! Statics: sorts, static constructors, and static terms.

mod check;
mod reduce;
mod subst;
mod term;

pub use term::{
    BaseSort, IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId,
    StaticVar, StaticVarId, Statics,
};
