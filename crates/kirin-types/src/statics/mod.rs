//! Statics: sorts, static constructors, and static terms.

mod check;
mod reduce;
mod sig;
mod subst;
mod term;

pub use sig::TypeSig;
pub use term::{
    IndexSort, Project, Sort, SortValue, StaticCon, StaticSig, StaticTerm, StaticTermId, StaticVar,
    StaticVarId, Statics,
};
