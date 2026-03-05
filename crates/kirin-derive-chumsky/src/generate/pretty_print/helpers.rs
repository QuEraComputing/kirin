use indexmap::IndexMap;

use crate::PrettyPrintLayout;
use kirin_derive_toolkit::ir::fields::FieldInfo;
use kirin_lexer::Token;

pub(super) fn build_field_map(
    collected: &[FieldInfo<PrettyPrintLayout>],
) -> IndexMap<String, (usize, &FieldInfo<PrettyPrintLayout>)> {
    let mut map = IndexMap::new();
    for (idx, field) in collected.iter().enumerate() {
        map.insert(field.index.to_string(), (idx, field));

        if let Some(ident) = &field.ident {
            map.insert(ident.to_string(), (idx, field));
        }
    }
    map
}

pub(super) fn tokens_to_string_with_spacing(
    tokens: &[Token],
    add_leading_space: bool,
    add_trailing_space: bool,
) -> String {
    let mut result = String::new();

    if add_leading_space && !tokens.is_empty() {
        let needs_leading_space = !matches!(
            tokens.first(),
            Some(Token::Comma) | Some(Token::RBrace) | Some(Token::RParen) | Some(Token::RBracket)
        );
        if needs_leading_space {
            result.push(' ');
        }
    }

    for (i, token) in tokens.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        match token {
            Token::EscapedLBrace => result.push('{'),
            Token::EscapedRBrace => result.push('}'),
            other => result.push_str(&other.to_string()),
        }
    }

    if add_trailing_space && !tokens.is_empty() {
        let needs_trailing_space = !matches!(
            tokens.last(),
            Some(Token::LBrace) | Some(Token::LParen) | Some(Token::LBracket)
        );
        if needs_trailing_space {
            result.push(' ');
        }
    }

    result
}
