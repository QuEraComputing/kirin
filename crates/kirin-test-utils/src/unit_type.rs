use kirin_ir::{HasBottom, HasTop, Lattice, TypeLattice};

/// A minimal type lattice with a single value.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Default)]
pub struct UnitType;

impl std::fmt::Display for UnitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "()")
    }
}

impl Lattice for UnitType {
    fn join(&self, _: &Self) -> Self {
        UnitType
    }

    fn meet(&self, _: &Self) -> Self {
        UnitType
    }

    fn is_subseteq(&self, _: &Self) -> bool {
        true
    }
}

impl HasBottom for UnitType {
    fn bottom() -> Self {
        UnitType
    }
}

impl HasTop for UnitType {
    fn top() -> Self {
        UnitType
    }
}

impl TypeLattice for UnitType {}

#[cfg(feature = "parser")]
mod parser_impls {
    use super::UnitType;
    use kirin_chumsky::chumsky::prelude::*;
    use kirin_chumsky::{BoxedParser, DirectlyParsable, HasParser, TokenInput};
    use kirin_lexer::Token;

    impl DirectlyParsable for UnitType {}

    impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for UnitType {
        type Output = UnitType;

        fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
        where
            I: TokenInput<'tokens, 'src>,
        {
            just(Token::LParen)
                .ignore_then(just(Token::RParen))
                .to(UnitType)
                .boxed()
        }
    }
}

#[cfg(feature = "pretty")]
mod pretty_impl {
    use super::UnitType;
    use kirin_prettyless::{ArenaDoc, DocAllocator, Document, PrettyPrint};

    impl PrettyPrint for UnitType {
        fn pretty_print<'a, L: kirin_ir::Dialect + PrettyPrint>(
            &self,
            doc: &'a Document<'a, L>,
        ) -> ArenaDoc<'a>
        where
            L::Type: std::fmt::Display,
        {
            doc.text("()")
        }
    }
}
