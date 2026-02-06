//! Tests for compile-time value fields (non-IR types with HasParser).

use chumsky::prelude::*;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::prelude::{ArenaDoc, DocAllocator, Document};
use kirin_chumsky::{BoxedParser, HasParser, PrettyPrint, TokenInput, parse_ast};
use kirin_lexer::Token;
use kirin_test_utils::SimpleType;

/// A custom compile-time value type that parses any identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Opcode(pub String);

impl std::fmt::Display for Opcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for Opcode {
    type Output = Opcode;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier(name) => Opcode(name.to_string())
        }
        .labelled("opcode")
        .boxed()
    }
}

impl PrettyPrint for Opcode {
    fn pretty_print<'a, L: kirin::ir::Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.0.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum ValueLang {
    #[chumsky(format = "{res:name} = apply {op} {arg} -> {res:type}")]
    Apply {
        res: ResultValue,
        op: Opcode,
        arg: SSAValue,
    },
}

#[test]
fn test_compile_time_value() {
    let ast = parse_ast::<ValueLang>("%r = apply custom_op %x -> i32").expect("parse failed");
    match ast.0 {
        ValueLangAST::Apply { res, op, arg, .. } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(op, Opcode("custom_op".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}

#[test]
fn test_compile_time_value_different() {
    let ast = parse_ast::<ValueLang>("%r = apply another %x -> f32").expect("parse failed");
    match ast.0 {
        ValueLangAST::Apply { res, op, arg, .. } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(op, Opcode("another".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}
