//! Test that a clear error message is shown when TypeLattice lacks HasParser.
//!
//! This test should fail to compile with a clear error message.

use kirin::ir::{Dialect, FiniteLattice, Lattice, ResultValue, SSAValue, TypeLattice};
use kirin_chumsky::{HasParser, PrettyPrint};

/// A type lattice that does NOT implement HasParser.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NoParserType {
    Int,
    Float,
}

impl Lattice for NoParserType {
    fn join(&self, other: &Self) -> Self {
        if self == other { self.clone() } else { NoParserType::Int }
    }
    fn meet(&self, other: &Self) -> Self {
        if self == other { self.clone() } else { NoParserType::Int }
    }
    fn is_subseteq(&self, other: &Self) -> bool {
        self == other
    }
}

impl FiniteLattice for NoParserType {
    fn bottom() -> Self { NoParserType::Int }
    fn top() -> Self { NoParserType::Int }
}

impl TypeLattice for NoParserType {}

impl std::fmt::Display for NoParserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NoParserType::Int => write!(f, "int"),
            NoParserType::Float => write!(f, "float"),
        }
    }
}

// This should fail to compile because NoParserType doesn't implement HasParser
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type_lattice = NoParserType)]
#[chumsky(crate = kirin_chumsky)]
pub enum BadLang {
    #[chumsky(format = "{res:name} = add {lhs} {rhs}")]
    Add { res: ResultValue, lhs: SSAValue, rhs: SSAValue },
}

fn main() {}
