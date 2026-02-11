use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

use kirin::ir::{Dialect, Typeof};
use kirin::parsers::chumsky::prelude::*;
use kirin::parsers::{BoxedParser, DirectlyParsable, HasParser, PrettyPrint, Token, TokenInput};
use kirin::pretty::{ArenaDoc, DocAllocator, Document};

use super::ArithType;

/// Built-in compile-time numeric values paired with [`ArithType`].
///
/// This value enum is intended as a practical default for constants and simple
/// compile-time evaluation. It keeps a one-to-one mapping with `ArithType` via
/// `Typeof<ArithType>`.
///
/// Parsing uses a simple heuristic:
/// - integer literals parse as `I64`
/// - float literals parse as `F64`
///
/// # Usage
///
/// ```rust,ignore
/// use kirin::ir::Typeof;
/// use kirin::parsers::parse_ast;
/// use kirin_arith::{ArithType, ArithValue};
///
/// let v = parse_ast::<ArithValue>("42").unwrap();
/// assert_eq!(v, ArithValue::I64(42));
/// assert_eq!(v.type_of(), ArithType::I64);
/// ```
///
/// If your language needs different literal defaults or value semantics, define
/// your own value enum and pair it with your own type lattice.
#[derive(Clone, Debug)]
pub enum ArithValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(f32),
    F64(f64),
}

impl PartialEq for ArithValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ArithValue::I8(lhs), ArithValue::I8(rhs)) => lhs == rhs,
            (ArithValue::I16(lhs), ArithValue::I16(rhs)) => lhs == rhs,
            (ArithValue::I32(lhs), ArithValue::I32(rhs)) => lhs == rhs,
            (ArithValue::I64(lhs), ArithValue::I64(rhs)) => lhs == rhs,
            (ArithValue::I128(lhs), ArithValue::I128(rhs)) => lhs == rhs,
            (ArithValue::U8(lhs), ArithValue::U8(rhs)) => lhs == rhs,
            (ArithValue::U16(lhs), ArithValue::U16(rhs)) => lhs == rhs,
            (ArithValue::U32(lhs), ArithValue::U32(rhs)) => lhs == rhs,
            (ArithValue::U64(lhs), ArithValue::U64(rhs)) => lhs == rhs,
            (ArithValue::U128(lhs), ArithValue::U128(rhs)) => lhs == rhs,
            (ArithValue::F32(lhs), ArithValue::F32(rhs)) => lhs.to_bits() == rhs.to_bits(),
            (ArithValue::F64(lhs), ArithValue::F64(rhs)) => lhs.to_bits() == rhs.to_bits(),
            _ => false,
        }
    }
}

impl Eq for ArithValue {}

impl Hash for ArithValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ArithValue::I8(value) => {
                0_u8.hash(state);
                value.hash(state);
            }
            ArithValue::I16(value) => {
                1_u8.hash(state);
                value.hash(state);
            }
            ArithValue::I32(value) => {
                2_u8.hash(state);
                value.hash(state);
            }
            ArithValue::I64(value) => {
                3_u8.hash(state);
                value.hash(state);
            }
            ArithValue::I128(value) => {
                4_u8.hash(state);
                value.hash(state);
            }
            ArithValue::U8(value) => {
                5_u8.hash(state);
                value.hash(state);
            }
            ArithValue::U16(value) => {
                6_u8.hash(state);
                value.hash(state);
            }
            ArithValue::U32(value) => {
                7_u8.hash(state);
                value.hash(state);
            }
            ArithValue::U64(value) => {
                8_u8.hash(state);
                value.hash(state);
            }
            ArithValue::U128(value) => {
                9_u8.hash(state);
                value.hash(state);
            }
            ArithValue::F32(value) => {
                10_u8.hash(state);
                value.to_bits().hash(state);
            }
            ArithValue::F64(value) => {
                11_u8.hash(state);
                value.to_bits().hash(state);
            }
        }
    }
}

impl Display for ArithValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ArithValue::I8(value) => write!(f, "{value}"),
            ArithValue::I16(value) => write!(f, "{value}"),
            ArithValue::I32(value) => write!(f, "{value}"),
            ArithValue::I64(value) => write!(f, "{value}"),
            ArithValue::I128(value) => write!(f, "{value}"),
            ArithValue::U8(value) => write!(f, "{value}"),
            ArithValue::U16(value) => write!(f, "{value}"),
            ArithValue::U32(value) => write!(f, "{value}"),
            ArithValue::U64(value) => write!(f, "{value}"),
            ArithValue::U128(value) => write!(f, "{value}"),
            ArithValue::F32(value) => {
                if value.fract() == 0.0 {
                    write!(f, "{value:.1}")
                } else {
                    write!(f, "{value}")
                }
            }
            ArithValue::F64(value) => {
                if value.fract() == 0.0 {
                    write!(f, "{value:.1}")
                } else {
                    write!(f, "{value}")
                }
            }
        }
    }
}

impl Typeof<ArithType> for ArithValue {
    fn type_of(&self) -> ArithType {
        match self {
            ArithValue::I8(_) => ArithType::I8,
            ArithValue::I16(_) => ArithType::I16,
            ArithValue::I32(_) => ArithType::I32,
            ArithValue::I64(_) => ArithType::I64,
            ArithValue::I128(_) => ArithType::I128,
            ArithValue::U8(_) => ArithType::U8,
            ArithValue::U16(_) => ArithType::U16,
            ArithValue::U32(_) => ArithType::U32,
            ArithValue::U64(_) => ArithType::U64,
            ArithValue::U128(_) => ArithType::U128,
            ArithValue::F32(_) => ArithType::F32,
            ArithValue::F64(_) => ArithType::F64,
        }
    }
}

impl DirectlyParsable for ArithValue {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for ArithValue {
    type Output = ArithValue;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        let integer =
            select! { Token::Int(value) = e => (value, e.span()) }.try_map(|(value, span), _| {
                value.parse::<i64>().map(ArithValue::I64).map_err(|_| {
                    Rich::custom(span, format!("invalid i64 literal for ArithValue: {value}"))
                })
            });

        let float =
            select! { Token::Float(value) = e => (value, e.span()) }.try_map(|(value, span), _| {
                value.parse::<f64>().map(ArithValue::F64).map_err(|_| {
                    Rich::custom(span, format!("invalid f64 literal for ArithValue: {value}"))
                })
            });

        float.or(integer).labelled("arith value").boxed()
    }
}

impl PrettyPrint for ArithValue {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(self.to_string())
    }
}
