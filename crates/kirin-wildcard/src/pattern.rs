use kirin::prelude::*;

use crate::{PatternLanguage, PatternStatement, Rewrite};

/// Holds one pattern in a single block: the statements to match, terminated
/// by a [`Rewrite`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$pattern {name} {body}")]
#[kirin(builders, type = T)]
pub struct Pattern<T: CompileTimeValue> {
    name: Symbol,
    body: Block,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Pattern<T> {
    /// The name identifying the pattern.
    pub fn name(&self) -> Symbol {
        self.name
    }

    /// The match statements, terminated by a [`Rewrite`].
    pub fn body(&self) -> Block {
        self.body
    }

    /// The [`Rewrite`] terminating the pattern body, or `None` if the body
    /// does not end in one.
    pub fn rewrite<'a, L: Dialect<Type = T>>(
        &self,
        stage: &'a StageInfo<PatternLanguage<L>>,
    ) -> Option<&'a Rewrite<T>> {
        match self.body.terminator(stage)?.definition(stage) {
            PatternLanguage::Wildcard(PatternStatement::Rewrite(rewrite)) => Some(rewrite),
            _ => None,
        }
    }

    /// The statement the match is rooted at: the one at [`Rewrite::root`]'s
    /// position in the pattern body. `None` if the body does not end in a
    /// [`Rewrite`] or the position is past the end of the body.
    pub fn root<L: Dialect<Type = T>>(
        &self,
        stage: &StageInfo<PatternLanguage<L>>,
    ) -> Option<Statement> {
        let rewrite = self.rewrite(stage)?;
        self.body.statements(stage).nth(rewrite.root())
    }
}
