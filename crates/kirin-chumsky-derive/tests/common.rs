//! Shared utilities for kirin-chumsky-derive integration tests.

use chumsky::prelude::*;
use kirin::ir::{FiniteLattice, Lattice, TypeLattice};
use kirin_chumsky::{BoxedParser, HasParser, TokenInput};
use kirin_lexer::Token;

/// Simple type lattice used across all parser tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleType {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Unit,
}

impl Lattice for SimpleType {
    fn join(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            SimpleType::Unit
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self == other || matches!(other, SimpleType::Unit)
    }
}

impl FiniteLattice for SimpleType {
    fn bottom() -> Self {
        SimpleType::Unit
    }

    fn top() -> Self {
        SimpleType::Unit
    }
}

impl TypeLattice for SimpleType {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for SimpleType {
    type Output = SimpleType;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier("i32") => SimpleType::I32,
            Token::Identifier("i64") => SimpleType::I64,
            Token::Identifier("f32") => SimpleType::F32,
            Token::Identifier("f64") => SimpleType::F64,
            Token::Identifier("bool") => SimpleType::Bool,
            Token::Identifier("unit") => SimpleType::Unit,
        }
        .labelled("type")
        .boxed()
    }
}
