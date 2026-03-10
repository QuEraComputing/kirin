use chumsky::prelude::*;
use kirin_lexer::Token;

use crate::traits::{BoxedParser, DirectlyParsable, HasParser, TokenInput};

impl<'t> HasParser<'t> for bool {
    type Output = bool;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
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

impl<'t> HasParser<'t> for String {
    type Output = String;

    fn parser<I>() -> BoxedParser<'t, I, Self::Output>
    where
        I: TokenInput<'t>,
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
