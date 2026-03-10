use chumsky::prelude::*;
use kirin_lexer::Token;

use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};

/// Creates a parser for signed integers.
pub(super) fn signed_int_parser<'t, T, I>(type_name: &'static str) -> BoxedParser<'t, I, T>
where
    I: TokenInput<'t>,
    T: std::str::FromStr + Clone + PartialEq + 't,
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
pub(super) fn unsigned_int_parser<'t, T, I>(type_name: &'static str) -> BoxedParser<'t, I, T>
where
    I: TokenInput<'t>,
    T: std::str::FromStr + Clone + PartialEq + 't,
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

// ============================================================================
// Signed integer implementations
// ============================================================================

impl<'t> HasParser<'t> for i8 {
    type Output = i8;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        signed_int_parser("i8")
    }
}

impl DirectlyParsable for i8 {}

impl<'t> HasParser<'t> for i16 {
    type Output = i16;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        signed_int_parser("i16")
    }
}

impl DirectlyParsable for i16 {}

impl<'t> HasParser<'t> for i32 {
    type Output = i32;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        signed_int_parser("i32")
    }
}

impl DirectlyParsable for i32 {}

impl<'t> HasParser<'t> for i64 {
    type Output = i64;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        signed_int_parser("i64")
    }
}

impl DirectlyParsable for i64 {}

impl<'t> HasParser<'t> for isize {
    type Output = isize;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        signed_int_parser("isize")
    }
}

impl DirectlyParsable for isize {}

// ============================================================================
// Unsigned integer implementations
// ============================================================================

impl<'t> HasParser<'t> for u8 {
    type Output = u8;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsigned_int_parser("u8")
    }
}

impl DirectlyParsable for u8 {}

impl<'t> HasParser<'t> for u16 {
    type Output = u16;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsigned_int_parser("u16")
    }
}

impl DirectlyParsable for u16 {}

impl<'t> HasParser<'t> for u32 {
    type Output = u32;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsigned_int_parser("u32")
    }
}

impl DirectlyParsable for u32 {}

impl<'t> HasParser<'t> for u64 {
    type Output = u64;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsigned_int_parser("u64")
    }
}

impl DirectlyParsable for u64 {}

impl<'t> HasParser<'t> for usize {
    type Output = usize;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
    {
        unsigned_int_parser("usize")
    }
}

impl DirectlyParsable for usize {}
