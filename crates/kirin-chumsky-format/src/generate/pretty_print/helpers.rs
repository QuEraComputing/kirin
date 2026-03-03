use indexmap::IndexMap;

use crate::PrettyPrintLayout;
use kirin_derive_core::ir::fields::FieldInfo;
use kirin_lexer::Token;

/// Build a map from field name/index (string) to (index, FieldInfo)
///
/// For named fields, both the field name and its index are added as keys.
/// This allows format strings to use either `{field_name}` or `{0}` syntax.
pub(super) fn build_field_map(
    collected: &[FieldInfo<PrettyPrintLayout>],
) -> IndexMap<String, (usize, &FieldInfo<PrettyPrintLayout>)> {
    let mut map = IndexMap::new();
    for (idx, field) in collected.iter().enumerate() {
        // Always add the index as a key (for {0}, {1}, etc. syntax)
        map.insert(field.index.to_string(), (idx, field));

        // Also add the name if it's a named field (for {field_name} syntax)
        if let Some(ident) = &field.ident {
            map.insert(ident.to_string(), (idx, field));
        }
    }
    map
}

/// Convert a sequence of tokens to a string for printing with proper spacing.
///
/// - `add_leading_space`: Add a space before the first token
/// - `add_trailing_space`: Add a space after the last token
pub(super) fn tokens_to_string_with_spacing(
    tokens: &[Token],
    add_leading_space: bool,
    add_trailing_space: bool,
) -> String {
    let mut result = String::new();

    // Add leading space if preceded by a field
    if add_leading_space && !tokens.is_empty() {
        // Check if the first token is a punctuation that typically doesn't want leading space
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
        // Use Display impl for most tokens, special-case escaped braces
        match token {
            Token::EscapedLBrace => result.push('{'),
            Token::EscapedRBrace => result.push('}'),
            other => result.push_str(&other.to_string()),
        }
    }

    // Add trailing space if followed by a field
    if add_trailing_space && !tokens.is_empty() {
        // Check if the last token is a punctuation that typically doesn't want trailing space
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
