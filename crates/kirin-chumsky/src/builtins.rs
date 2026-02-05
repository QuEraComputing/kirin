//! `HasParser` implementations for common builtin types.
//!
//! This module provides parser support for:
//! - Signed integers: `i8`, `i16`, `i32`, `i64`, `isize`
//! - Unsigned integers: `u8`, `u16`, `u32`, `u64`, `usize`
//! - Floating point: `f32`, `f64`
//! - Boolean: `bool`
//! - String: `String`
//!
//! Note: `PrettyPrint` implementations for these types are in `kirin-prettyless`
//! due to the orphan rule.

use chumsky::prelude::*;
use kirin_lexer::Token;

use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};

// ============================================================================
// Integer parsing helpers
// ============================================================================

/// Creates a parser for signed integers.
fn signed_int_parser<'tokens, 'src: 'tokens, T, I>(
    type_name: &'static str,
) -> BoxedParser<'tokens, 'src, I, T>
where
    I: TokenInput<'tokens, 'src>,
    T: std::str::FromStr + Clone + PartialEq + 'tokens,
{
    select! { Token::Int(v) = e => (v, e.span()) }
        .try_map(move |(v, span), _| {
            v.parse::<T>()
                .map_err(|_| Rich::custom(span, format!("invalid {} literal: {}", type_name, v)))
        })
        .labelled(type_name)
        .boxed()
}

/// Creates a parser for unsigned integers (accepts both decimal and hex).
fn unsigned_int_parser<'tokens, 'src: 'tokens, T, I>(
    type_name: &'static str,
) -> BoxedParser<'tokens, 'src, I, T>
where
    I: TokenInput<'tokens, 'src>,
    T: std::str::FromStr + Clone + PartialEq + 'tokens,
    T: num_traits::Num,
{
    let decimal = select! { Token::Int(v) = e => (v, e.span(), false) };
    let hex = select! { Token::Unsigned(v) = e => (v, e.span(), true) };

    decimal
        .or(hex)
        .try_map(move |(v, span, is_hex), _| {
            if is_hex {
                T::from_str_radix(v, 16).map_err(|_| {
                    Rich::custom(span, format!("invalid {} hex literal: 0x{}", type_name, v))
                })
            } else {
                v.parse::<T>().map_err(|_| {
                    Rich::custom(span, format!("invalid {} literal: {}", type_name, v))
                })
            }
        })
        .labelled(type_name)
        .boxed()
}

/// Creates a parser for floating point numbers.
fn float_parser<'tokens, 'src: 'tokens, T, I>(
    type_name: &'static str,
) -> BoxedParser<'tokens, 'src, I, T>
where
    I: TokenInput<'tokens, 'src>,
    T: std::str::FromStr + Clone + PartialEq + 'tokens,
{
    // Accept both Float tokens and Int tokens (for cases like "1" meaning 1.0)
    let float_token = select! { Token::Float(v) = e => (v, e.span()) };
    let int_token = select! { Token::Int(v) = e => (v, e.span()) };

    float_token
        .or(int_token)
        .try_map(move |(v, span), _| {
            v.parse::<T>()
                .map_err(|_| Rich::custom(span, format!("invalid {} literal: {}", type_name, v)))
        })
        .labelled(type_name)
        .boxed()
}

// ============================================================================
// Signed integer implementations
// ============================================================================

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for i8 {
    type Output = i8;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        signed_int_parser("i8")
    }
}

impl DirectlyParsable for i8 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for i16 {
    type Output = i16;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        signed_int_parser("i16")
    }
}

impl DirectlyParsable for i16 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for i32 {
    type Output = i32;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        signed_int_parser("i32")
    }
}

impl DirectlyParsable for i32 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for i64 {
    type Output = i64;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        signed_int_parser("i64")
    }
}

impl DirectlyParsable for i64 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for isize {
    type Output = isize;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        signed_int_parser("isize")
    }
}

impl DirectlyParsable for isize {}

// ============================================================================
// Unsigned integer implementations
// ============================================================================

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for u8 {
    type Output = u8;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        unsigned_int_parser("u8")
    }
}

impl DirectlyParsable for u8 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for u16 {
    type Output = u16;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        unsigned_int_parser("u16")
    }
}

impl DirectlyParsable for u16 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for u32 {
    type Output = u32;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        unsigned_int_parser("u32")
    }
}

impl DirectlyParsable for u32 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for u64 {
    type Output = u64;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        unsigned_int_parser("u64")
    }
}

impl DirectlyParsable for u64 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for usize {
    type Output = usize;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        unsigned_int_parser("usize")
    }
}

impl DirectlyParsable for usize {}

// ============================================================================
// Floating point implementations
// ============================================================================

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for f32 {
    type Output = f32;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        float_parser("f32")
    }
}

impl DirectlyParsable for f32 {}

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for f64 {
    type Output = f64;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        float_parser("f64")
    }
}

impl DirectlyParsable for f64 {}

// ============================================================================
// Boolean implementation
// ============================================================================

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for bool {
    type Output = bool;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        select! {
            Token::Identifier("true") => true,
            Token::Identifier("false") => false,
        }
        .labelled("bool")
        .boxed()
    }
}

impl DirectlyParsable for bool {}

// ============================================================================
// String implementation
// ============================================================================

impl<'tokens, 'src: 'tokens> HasParser<'tokens, 'src> for String {
    type Output = String;

    fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
    where
        I: TokenInput<'tokens, 'src>,
    {
        // Accept both string literals and identifiers
        select! {
            Token::StringLit(s) => {
                // Remove surrounding quotes if present
                if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    s[1..s.len()-1].to_string()
                } else {
                    s
                }
            },
            Token::Identifier(id) => id.to_string(),
        }
        .labelled("string")
        .boxed()
    }
}

impl DirectlyParsable for String {}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::input::Stream;
    use kirin_lexer::Logos;

    fn parse_with<T: HasParser<'static, 'static>>(input: &'static str) -> Result<T::Output, ()> {
        let tokens: Vec<_> = Token::lexer(input)
            .spanned()
            .map(|(tok, span)| {
                let token = tok.unwrap_or(Token::Error);
                (token, SimpleSpan::from(span))
            })
            .collect();

        let eoi = SimpleSpan::from(input.len()..input.len());
        let stream = Stream::from_iter(tokens).map(eoi, |(t, s)| (t, s));
        T::parser().parse(stream).into_result().map_err(|_| ())
    }

    #[test]
    fn test_parse_i32() {
        assert_eq!(parse_with::<i32>("42"), Ok(42));
        assert_eq!(parse_with::<i32>("-123"), Ok(-123));
        assert_eq!(parse_with::<i32>("0"), Ok(0));
    }

    #[test]
    fn test_parse_u32() {
        assert_eq!(parse_with::<u32>("42"), Ok(42));
        assert_eq!(parse_with::<u32>("0"), Ok(0));
        // Negative should fail for unsigned
        assert!(parse_with::<u32>("-1").is_err());
    }

    #[test]
    fn test_parse_u64_hex() {
        assert_eq!(parse_with::<u64>("0xff"), Ok(255));
        assert_eq!(parse_with::<u64>("0xDEADBEEF"), Ok(0xDEADBEEF));
    }

    #[test]
    fn test_parse_f64() {
        assert_eq!(parse_with::<f64>("3.14"), Ok(3.14));
        assert_eq!(parse_with::<f64>("1"), Ok(1.0));
        assert_eq!(parse_with::<f64>("-2.5"), Ok(-2.5));
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_with::<bool>("true"), Ok(true));
        assert_eq!(parse_with::<bool>("false"), Ok(false));
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(parse_with::<String>("hello"), Ok("hello".to_string()));
        assert_eq!(parse_with::<String>("\"quoted\""), Ok("quoted".to_string()));
    }
}
