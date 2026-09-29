//! A minimal dialect for the crate's own unit tests.
//!
//! Deliberately smaller than `tests/common.rs`'s `BuilderDialect`: it carries
//! only what the in-crate tests need — operands, so values have uses, and a
//! terminator, so blocks have a body to summarize. Successors, bodies, results,
//! and graph edges are covered by the integration tests, which reach the same
//! code through the public API.

use crate::{
    Block, BuilderStageInfo, CFG, DiGraph, Dialect, HasArguments, HasArgumentsMut, HasBlocks,
    HasBlocksMut, HasCFG, HasCFGMut, HasDigraphs, HasDigraphsMut, HasResults, HasResultsMut,
    HasSuccessors, HasSuccessorsMut, HasUngraphs, HasUngraphsMut, IsConstant, IsEdge, IsPure,
    IsSpeculatable, IsTerminator, Placeholder, ResultValue, SSAValue, Successor, UnGraph,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub(crate) enum TestType {
    #[default]
    Any,
    I32,
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::Any => write!(f, "any"),
            TestType::I32 => write!(f, "i32"),
        }
    }
}

impl Placeholder for TestType {
    fn placeholder() -> Self {
        Self::Any
    }
}

/// - `Nop`: no operands, no results
/// - `Add(a, b)` / `Use(a)`: operand slots, so a value can have several uses
/// - `Return`: a terminator
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TestLang {
    Nop,
    Add(SSAValue, SSAValue),
    Use(SSAValue),
    Return,
}

impl<'a> HasArguments<'a> for TestLang {
    type Iter = std::vec::IntoIter<&'a SSAValue>;
    fn arguments(&'a self) -> Self::Iter {
        match self {
            TestLang::Add(a, b) => vec![a, b].into_iter(),
            TestLang::Use(a) => vec![a].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl<'a> HasArgumentsMut<'a> for TestLang {
    type IterMut = std::vec::IntoIter<&'a mut SSAValue>;
    fn arguments_mut(&'a mut self) -> Self::IterMut {
        match self {
            TestLang::Add(a, b) => vec![a, b].into_iter(),
            TestLang::Use(a) => vec![a].into_iter(),
            _ => vec![].into_iter(),
        }
    }
}

impl IsTerminator for TestLang {
    fn is_terminator(&self) -> bool {
        matches!(self, TestLang::Return)
    }
}

/// The remaining `Dialect` supertraits, none of which this dialect exercises.
macro_rules! empty_impls {
    ($ty:ty) => {
        impl<'a> HasResults<'a> for $ty {
            type Iter = std::iter::Empty<&'a ResultValue>;
            fn results(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasResultsMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut ResultValue>;
            fn results_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl<'a> HasBlocks<'a> for $ty {
            type Iter = std::iter::Empty<&'a Block>;
            fn blocks(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasBlocksMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut Block>;
            fn blocks_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl<'a> HasCFG<'a> for $ty {
            type Iter = std::iter::Empty<&'a CFG>;
            fn cfgs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasCFGMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut CFG>;
            fn cfgs_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl<'a> HasDigraphs<'a> for $ty {
            type Iter = std::iter::Empty<&'a DiGraph>;
            fn digraphs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasDigraphsMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut DiGraph>;
            fn digraphs_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl<'a> HasUngraphs<'a> for $ty {
            type Iter = std::iter::Empty<&'a UnGraph>;
            fn ungraphs(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasUngraphsMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut UnGraph>;
            fn ungraphs_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl<'a> HasSuccessors<'a> for $ty {
            type Iter = std::iter::Empty<&'a Successor>;
            fn successors(&'a self) -> Self::Iter {
                std::iter::empty()
            }
        }
        impl<'a> HasSuccessorsMut<'a> for $ty {
            type IterMut = std::iter::Empty<&'a mut Successor>;
            fn successors_mut(&'a mut self) -> Self::IterMut {
                std::iter::empty()
            }
        }
        impl IsConstant for $ty {
            fn is_constant(&self) -> bool {
                false
            }
        }
        impl IsPure for $ty {
            fn is_pure(&self) -> bool {
                true
            }
        }
        impl IsSpeculatable for $ty {
            fn is_speculatable(&self) -> bool {
                true
            }
        }
        impl IsEdge for $ty {
            fn is_edge(&self) -> bool {
                false
            }
        }
    };
}

empty_impls!(TestLang);

impl Dialect for TestLang {
    type Type = TestType;
}

/// An empty builder stage over [`TestLang`].
pub(crate) fn new_stage() -> BuilderStageInfo<TestLang> {
    BuilderStageInfo::default()
}
