use std::fmt::{Display, Formatter};

use kirin::ir::Dialect;
use kirin::parsers::chumsky::prelude::*;
use kirin::parsers::{BoxedParser, DirectlyParsable, HasParser, PrettyPrint, Token, TokenInput};
use kirin::pretty::{ArenaDoc, DocAllocator, Document};

/// Built-in arithmetic type lattice for `kirin-arith`.
///
/// This enum mirrors Rust primitive numeric type names in textual form (`i32`,
/// `u64`, `f64`, ...), making round-trip parse/print straightforward.
///
/// # Usage
///
/// ```rust,ignore
/// use kirin::parsers::parse_ast;
/// use kirin_arith::ArithType;
///
/// let ty = parse_ast::<ArithType>("i32").unwrap();
/// assert_eq!(ty, ArithType::I32);
/// assert_eq!(ty.to_string(), "i32");
/// ```
///
/// If this built-in lattice is not sufficient, define your own type enum and
/// use `Arith<YourType>` to preserve your language semantics.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ArithType {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
}

impl Default for ArithType {
    fn default() -> Self {
        Self::I64
    }
}

impl Display for ArithType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ArithType::I8 => "i8",
            ArithType::I16 => "i16",
            ArithType::I32 => "i32",
            ArithType::I64 => "i64",
            ArithType::I128 => "i128",
            ArithType::U8 => "u8",
            ArithType::U16 => "u16",
            ArithType::U32 => "u32",
            ArithType::U64 => "u64",
            ArithType::U128 => "u128",
            ArithType::F32 => "f32",
            ArithType::F64 => "f64",
        };

        f.write_str(name)
    }
}

impl DirectlyParsable for ArithType {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for ArithType {
    type Output = ArithType;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier("i8") => ArithType::I8,
            Token::Identifier("i16") => ArithType::I16,
            Token::Identifier("i32") => ArithType::I32,
            Token::Identifier("i64") => ArithType::I64,
            Token::Identifier("i128") => ArithType::I128,
            Token::Identifier("u8") => ArithType::U8,
            Token::Identifier("u16") => ArithType::U16,
            Token::Identifier("u32") => ArithType::U32,
            Token::Identifier("u64") => ArithType::U64,
            Token::Identifier("u128") => ArithType::U128,
            Token::Identifier("f32") => ArithType::F32,
            Token::Identifier("f64") => ArithType::F64,
        }
        .labelled("arith type")
        .boxed()
    }
}

impl PrettyPrint for ArithType {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
