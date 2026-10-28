use kirin::prelude::*;

use crate::{
    Advance, Constraint, Erase, Hole, Interrupt, NativeCall, Pattern, PatternStatement, Replace,
    Rewrite, Skip, WalkResultOf,
};

/// The language a pattern is written in: the ops of `L`, reused as-is, plus
/// the [`PatternStatement`]s.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
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
