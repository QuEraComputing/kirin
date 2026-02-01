//! Tests for compile-time value fields (non-IR types with HasParser).

mod common;

use chumsky::prelude::*;
use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{parse, BoxedParser, HasParser, TokenInput};
use kirin_chumsky_derive::{HasRecursiveParser, WithAbstractSyntaxTree};
use kirin_lexer::Token;

/// A custom compile-time value type that parses any identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Opcode(pub String);

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

#[derive(Debug, Clone, PartialEq, Dialect, HasRecursiveParser, WithAbstractSyntaxTree)]
#[kirin(type_lattice = SimpleType)]
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
    let ast = parse::<ValueLang>("%r = apply custom_op %x -> i32").expect("parse failed");
    match ast {
        ValueLangAST::Apply { res, op, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::I32));
            assert_eq!(op, Opcode("custom_op".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}

#[test]
fn test_compile_time_value_different() {
    let ast = parse::<ValueLang>("%r = apply another %x -> f32").expect("parse failed");
    match ast {
        ValueLangAST::Apply { res, op, arg } => {
            assert_eq!(res.name.value, "r");
            assert_eq!(res.ty, Some(SimpleType::F32));
            assert_eq!(op, Opcode("another".to_string()));
            assert_eq!(arg.name.value, "x");
        }
    }
}
