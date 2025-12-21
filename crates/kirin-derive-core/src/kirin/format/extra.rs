// use crate::{kirin::attrs::KirinStructOptions, prelude::*};
// use darling::FromDeriveInput;
// use kirin_lexer::Token;

// use super::context::Format;

// pub struct StatementExtra<'src> {
//     pub format: Vec<Token<'src>>,
// }

// impl<'src> ScanExtra<'src, syn::DeriveInput, StatementExtra<'src>> for Format {
//     fn scan_extra(&self, node: &'src syn::DeriveInput) -> syn::Result<StatementExtra<'src>> {
//         let attrs = KirinStructOptions::from_derive_input(node)?;
//         let tokens = attrs
//             .format
//             .map(|s| kirin_lexer::lex(&s).collect::<Result<Vec<_>, String>>())
//             .unwrap_or_else(|| Ok(Vec::new()))
//             .map_err(|e| syn::Error::new_spanned(node, e))?;
//         Ok(StatementExtra { format: tokens })
//     }
// }
