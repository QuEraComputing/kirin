use chumsky::prelude::*;

use crate::{BoxedParser, HasParser, SymbolName, Token, TokenInput, identifier, symbol};

/// Only the container syntax is parsed here. Member spans are handed to the
/// dialect's existing statement parser, including any nested body syntax.
#[derive(Clone, PartialEq)]
pub(super) struct ModuleText<'t> {
    pub name: Option<SymbolName<'t>>,
    pub members: Vec<SimpleSpan>,
}

impl<'t> HasParser<'t> for ModuleText<'t> {
    type Output = Self;

    fn parser<I: TokenInput<'t>>() -> BoxedParser<'t, I, Self> {
        // A token tree keeps semicolons inside bodies, tuples, and lists from
        // splitting a member. The lexer already handles strings and comments.
        let tree = recursive(|tree| {
            let leaf = any().filter(|token| {
                !matches!(
                    token,
                    Token::LBrace
                        | Token::RBrace
                        | Token::LParen
                        | Token::RParen
                        | Token::LBracket
                        | Token::RBracket
                )
            });
            choice((
                leaf,
                tree.clone()
                    .repeated()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace))
                    .to(Token::LBrace),
                tree.clone()
                    .repeated()
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .to(Token::LParen),
                tree.repeated()
                    .delimited_by(just(Token::LBracket), just(Token::RBracket))
                    .to(Token::LBracket),
            ))
        });
        let member = tree
            .filter(|token| *token != Token::Semicolon)
            .repeated()
            .at_least(1)
            .to_span()
            .then_ignore(just(Token::Semicolon));

        identifier("module")
            .ignore_then(symbol().or_not())
            .then(
                member
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(name, members)| Self { name, members })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleText;
    use crate::parse_ast;

    #[test]
    fn member_spans_keep_nested_delimiters_strings_and_comments() {
        let input = r#"module @m {
            first "a; } b" [1, (2, 3)] { nested; }; // ignored ; }
            second;
        }"#;
        let ast = parse_ast::<ModuleText<'_>>(input).unwrap();
        let members: Vec<_> = ast
            .members
            .iter()
            .map(|span| &input[span.start..span.end])
            .collect();
        assert_eq!(
            members,
            [r#"first "a; } b" [1, (2, 3)] { nested; }"#, "second"]
        );
    }
}
