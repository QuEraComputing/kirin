use kirin_ir::{FiniteLattice, Lattice, TypeLattice};

/// Simple type lattice used for parser integration tests.
///
/// This type has concrete variants (`i32`, `i64`, `f32`, `f64`, `bool`, `unit`)
/// compared to `SimpleIRType` which uses abstract categories (`Int`, `Float`, etc).
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

impl Default for SimpleType {
    fn default() -> Self {
        SimpleType::Unit
    }
}

impl std::fmt::Display for SimpleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleType::I32 => write!(f, "i32"),
            SimpleType::I64 => write!(f, "i64"),
            SimpleType::F32 => write!(f, "f32"),
            SimpleType::F64 => write!(f, "f64"),
            SimpleType::Bool => write!(f, "bool"),
            SimpleType::Unit => write!(f, "unit"),
        }
    }
}

#[cfg(feature = "parser")]
mod parser_impls {
    use super::SimpleType;
    use kirin_chumsky::chumsky::prelude::*;
    use kirin_chumsky::{BoxedParser, DirectlyParsable, HasParser, TokenInput};
    use kirin_lexer::Token;

    impl DirectlyParsable for SimpleType {}

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
}
