use kirin_ir::{HasBottom, HasTop, Lattice, TypeLattice, Typeof};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleType {
    Any,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Unit,
    Bottom,
}

impl Lattice for SimpleType {
    fn is_subseteq(&self, other: &Self) -> bool {
        self == other || matches!(other, SimpleType::Any) || matches!(self, SimpleType::Bottom)
    }

    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            SimpleType::Any
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            SimpleType::Bottom
        }
    }
}

impl HasBottom for SimpleType {
    fn bottom() -> Self {
        SimpleType::Bottom
    }
}

impl HasTop for SimpleType {
    fn top() -> Self {
        SimpleType::Any
    }
}

impl TypeLattice for SimpleType {}

impl Default for SimpleType {
    fn default() -> Self {
        Self::bottom()
    }
}

impl std::fmt::Display for SimpleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleType::Any => write!(f, "any"),
            SimpleType::I32 => write!(f, "i32"),
            SimpleType::I64 => write!(f, "i64"),
            SimpleType::F32 => write!(f, "f32"),
            SimpleType::F64 => write!(f, "f64"),
            SimpleType::Bool => write!(f, "bool"),
            SimpleType::Unit => write!(f, "unit"),
            SimpleType::Bottom => write!(f, "bottom"),
        }
    }
}

impl Typeof<SimpleType> for i64 {
    fn type_of(&self) -> SimpleType {
        SimpleType::I64
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
                Token::Identifier("any") => SimpleType::Any,
                Token::Identifier("i32") => SimpleType::I32,
                Token::Identifier("i64") => SimpleType::I64,
                Token::Identifier("f32") => SimpleType::F32,
                Token::Identifier("f64") => SimpleType::F64,
                Token::Identifier("bool") => SimpleType::Bool,
                Token::Identifier("unit") => SimpleType::Unit,
                Token::Identifier("bottom") => SimpleType::Bottom,
            }
            .labelled("type")
            .boxed()
        }
    }
}
