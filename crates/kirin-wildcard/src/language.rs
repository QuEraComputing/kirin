use kirin::prelude::*;

use crate::{
    Advance, Constraint, Erase, Hole, Interrupt, NativeCall, Pattern, PatternStatement, Replace,
    Rewrite, Skip, WalkResultOf,
};

/// The language a pattern is written in: the ops of `L`, reused as-is, plus
/// the [`PatternStatement`]s.
///
/// Derives every property in Dialect except [`IsTerminator`]: a pattern body
/// is ended by its [`Rewrite`], so a matched terminator of `L` is an ordinary
/// statement of it. That is why the `Dialect` traits are derived one by one
/// instead of with `#[derive(Dialect)]`, which would delegate `is_terminator`
/// too. [`BranchOp`] has no standalone derive, so it is implemented by hand.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PrettyPrint,
    HasArguments,
    HasArgumentsMut,
    HasResults,
    HasResultsMut,
    HasSymbols,
    HasSymbolsMut,
    HasBlocks,
    HasBlocksMut,
    HasSuccessors,
    HasSuccessorsMut,
    HasCFG,
    HasCFGMut,
    HasDigraphs,
    HasDigraphsMut,
    HasUngraphs,
    HasUngraphsMut,
    IsConstant,
    IsPure,
    IsSpeculatable,
    IsEdge,
    LiftProject,
)]
#[wraps]
#[kirin(type = L::Type)]
pub enum PatternLanguage<L: Dialect> {
    Op(L),
    // Each wildcard op's generated builder (`Hole::new`, ...) requires
    // `From<Hole<_>>` on the stage's language; these bridge through
    // `PatternStatement`.
    #[wraps(lift_project_from(
        Pattern<L::Type>,
        Hole<L::Type>,
        NativeCall<L::Type>,
        Constraint<L::Type>,
        Replace<L::Type>,
        Erase<L::Type>,
        Rewrite<L::Type>,
        Advance<L::Type>,
        Skip<L::Type>,
        Interrupt<L::Type>,
        WalkResultOf<L::Type>
    ))]
    Wildcard(PatternStatement<L::Type>),
}

impl<L: Dialect> Dialect for PatternLanguage<L> {
    type Type = L::Type;
}

impl<L: Dialect> IsTerminator for PatternLanguage<L> {
    fn is_terminator(&self) -> bool {
        match self {
            Self::Op(_) => false,
            Self::Wildcard(statement) => statement.is_terminator(),
        }
    }
}

/// Forwards to the wrapped statement, matching the derived `HasSuccessors`.
impl<L: Dialect> BranchOp for PatternLanguage<L> {
    fn successor_operands(&self, index: usize) -> Option<&[SSAValue]> {
        match self {
            Self::Op(op) => op.successor_operands(index),
            Self::Wildcard(statement) => statement.successor_operands(index),
        }
    }

    fn successor_operands_mut(&mut self, index: usize) -> Option<&mut Vec<SSAValue>> {
        match self {
            Self::Op(op) => op.successor_operands_mut(index),
            Self::Wildcard(statement) => statement.successor_operands_mut(index),
        }
    }
}
