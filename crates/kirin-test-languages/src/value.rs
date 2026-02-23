use crate::SimpleType;
use kirin_ir::Typeof;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    I64(i64),
    F64(f64),
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::I64(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            Value::F64(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
            }
        }
    }
}

impl Typeof<SimpleType> for Value {
    fn type_of(&self) -> SimpleType {
        match self {
            Value::I64(_) => SimpleType::I64,
            Value::F64(_) => SimpleType::F64,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::I64(v) => write!(f, "{v}"),
            Value::F64(v) => write!(f, "{v}"),
        }
    }
}

#[cfg(feature = "parser")]
mod parser_impl {
    use super::Value;
    use kirin_chumsky::chumsky::prelude::*;
    use kirin_chumsky::{BoxedParser, DirectlyParsable, HasParser, TokenInput};
    use kirin_lexer::Token;

    impl DirectlyParsable for Value {}

    impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for Value {
        type Output = Value;

        fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
        where
            I: TokenInput<'tokens, 'src>,
        {
            let integer = select! { Token::Int(value) = e => (value, e.span()) }.try_map(
                |(value, span), _| {
                    value.parse::<i64>().map(Value::I64).map_err(|_| {
                        Rich::custom(span, format!("invalid i64 literal: {value}"))
                    })
                },
            );

            let float = select! { Token::Float(value) = e => (value, e.span()) }.try_map(
                |(value, span), _| {
                    value.parse::<f64>().map(Value::F64).map_err(|_| {
                        Rich::custom(span, format!("invalid f64 literal: {value}"))
                    })
                },
            );

            float.or(integer).labelled("value").boxed()
        }
    }
}
