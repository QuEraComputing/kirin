//! Statics: sorts, static constructors, and static terms.

mod check;
mod subst;
mod term;

pub use term::{
    IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId, StaticVar,
    StaticVarId, Statics,
};
