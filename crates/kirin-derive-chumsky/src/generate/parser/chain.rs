//! Parser chain building and AST constructor generation.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_toolkit::ir::fields::{Collection, FieldInfo};

use crate::field_kind::FieldKind;
use crate::format::{Format, FormatElement, FormatOption};
use crate::validation::FieldOccurrence;

use super::GenerateHasDialectParser;

/// Represents a part of the parser chain.
pub enum ParserPart {
    /// A token parser (literal tokens to match)
    Token(TokenStream),
    /// A field parser (parses a field value)
    Field(TokenStream),
}

impl GenerateHasDialectParser {
    /// Builds a parser chain from the format string and field occurrences.
    pub(super) fn build_parser_chain(
        &self,
        format: &Format<'_>,
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
        ast_name: &syn::Ident,
        ir_type: &syn::Path,
        type_params: &[TokenStream],
    ) -> syn::Result<TokenStream> {
        let mut occurrence_iter = occurrences.iter();
        let mut parser_parts: Vec<ParserPart> = Vec::new();

        for elem in format.elements() {
            match elem {
                FormatElement::Token(tokens) => {
                    parser_parts.push(ParserPart::Token(self.token_parser(tokens)));
                }
                FormatElement::Field(_, _) => {
                    let occurrence = occurrence_iter
                        .next()
                        .expect("occurrence sequence mismatch");
                    parser_parts.push(ParserPart::Field(self.field_parser(
                        crate_path,
                        occurrence.field,
                        &occurrence.option,
                        ast_name,
                        ir_type,
                        type_params,
                    )));
                }
            }
        }

        if parser_parts.is_empty() {
            return Ok(quote! { #crate_path::chumsky::prelude::empty() });
        }

        let first_field_idx = parser_parts
            .iter()
            .position(|p| matches!(p, ParserPart::Field(_)));

        let mut parser_expr: Option<TokenStream> = None;

        for (i, part) in parser_parts.iter().enumerate() {
            match part {
                ParserPart::Token(tok_parser) => match &parser_expr {
                    Some(expr) => {
                        parser_expr = Some(quote! { #expr.then_ignore(#tok_parser) });
                    }
                    None => {
                        if first_field_idx.is_some() && i < first_field_idx.unwrap() {
                            continue;
                        } else {
                            parser_expr = Some(quote! { #tok_parser });
                        }
                    }
                },
                ParserPart::Field(field_parser) => match &parser_expr {
                    Some(expr) => {
                        parser_expr = Some(quote! { #expr.then(#field_parser) });
                    }
                    None => {
                        let preceding_tokens: Vec<_> = parser_parts[..i]
                            .iter()
                            .filter_map(|p| match p {
                                ParserPart::Token(t) => Some(t.clone()),
                                _ => None,
                            })
                            .collect();

                        if !preceding_tokens.is_empty() {
                            let mut combined = preceding_tokens[0].clone();
                            for tok in &preceding_tokens[1..] {
                                combined = quote! { #combined.then_ignore(#tok) };
                            }
                            parser_expr = Some(quote! { #combined.ignore_then(#field_parser) });
                        } else {
                            parser_expr = Some(field_parser.clone());
                        }
                    }
                },
            }
        }

        Ok(parser_expr.unwrap_or_else(|| quote! { #crate_path::chumsky::prelude::empty() }))
    }

    fn field_parser(
        &self,
        crate_path: &syn::Path,
        field: &FieldInfo<ChumskyLayout>,
        opt: &FormatOption,
        ast_name: &syn::Ident,
        ir_type: &syn::Path,
        type_params: &[TokenStream],
    ) -> TokenStream {
        let kind = FieldKind::from_field_info(field);
        let base = kind.parser_expr(crate_path, opt, ast_name, ir_type, type_params);
        match field.collection {
            Collection::Single => base,
            Collection::Vec => quote! {
                #base
                    .separated_by(#crate_path::chumsky::prelude::just(#crate_path::Token::Comma))
                    .allow_trailing()
                    .collect::<::std::vec::Vec<_>>()
            },
            Collection::Option => quote! { #base.or_not() },
        }
    }

    pub(super) fn ast_constructor(
        &self,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        collected: &[FieldInfo<ChumskyLayout>],
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
        type_params: &[TokenStream],
    ) -> TokenStream {
        let mut field_occurrences: HashMap<usize, Vec<&FieldOccurrence>> = HashMap::new();
        for occ in occurrences {
            field_occurrences
                .entry(occ.field.index)
                .or_default()
                .push(occ);
        }

        let ast_fields: Vec<_> = collected
            .iter()
            .filter(|f| field_occurrences.contains_key(&f.index) || !f.has_default())
            .collect();

        let has_named = ast_fields.first().and_then(|f| f.ident.as_ref()).is_some();

        let phantom_data = if type_params.is_empty() {
            quote! { ::core::marker::PhantomData::<fn() -> (&'tokens (), &'src (), __TypeOutput, __LanguageOutput)> }
        } else {
            quote! { ::core::marker::PhantomData::<fn() -> (&'tokens (), &'src (), #(#type_params,)* __TypeOutput, __LanguageOutput)> }
        };

        if has_named {
            let assigns = ast_fields.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let value = self.build_field_value(field, &field_occurrences, crate_path);
                quote! { #name: #value }
            });
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                None => quote! { #ast_name { #(#assigns,)* _marker: #phantom_data } },
            }
        } else {
            let mut sorted_ast_fields: Vec<_> = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let values = sorted_ast_fields
                .iter()
                .map(|field| self.build_field_value(field, &field_occurrences, crate_path));
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                None => quote! { #ast_name ( #(#values,)* #phantom_data ) },
            }
        }
    }

    fn build_field_value(
        &self,
        field: &FieldInfo<ChumskyLayout>,
        field_occurrences: &HashMap<usize, Vec<&FieldOccurrence>>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let occs = field_occurrences.get(&field.index);
        let kind = FieldKind::from_field_info(field);

        match occs {
            None => {
                unreachable!(
                    "field '{}' not in format string - this should have been caught earlier",
                    field
                )
            }
            Some(occs) if occs.len() == 1 => {
                let occ = occs[0];
                let var = &occ.var_name;

                match &occ.option {
                    FormatOption::Name => kind
                        .construct_from_name_only(crate_path, var)
                        .unwrap_or_else(|| quote! { #var }),
                    FormatOption::Type if kind.supports_name_type_options() => {
                        unreachable!(
                            "field '{}' has only :type occurrence - this should have been caught by validation",
                            field
                        )
                    }
                    _ => quote! { #var },
                }
            }
            Some(occs) => {
                let name_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Name));
                let type_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Type));

                match (name_occ, type_occ) {
                    (Some(name), Some(ty)) => kind
                        .construct_from_name_and_type(crate_path, &name.var_name, &ty.var_name)
                        .unwrap_or_else(|| {
                            let var = &occs[0].var_name;
                            quote! { #var }
                        }),
                    _ => {
                        let var = &occs[0].var_name;
                        quote! { #var }
                    }
                }
            }
        }
    }

    pub(super) fn token_parser(&self, tokens: &[kirin_lexer::Token<'_>]) -> TokenStream {
        let crate_path = &self.config.crate_path;
        let mut iter = tokens.iter();
        let Some(first) = iter.next() else {
            return quote! { #crate_path::chumsky::prelude::empty().ignored() };
        };
        let mut parser = quote! { #crate_path::chumsky::prelude::just(#first) };
        for tok in iter {
            parser = quote! { #parser.then_ignore(#crate_path::chumsky::prelude::just(#tok)) };
        }
        parser
    }
}

/// Builds a nested tuple pattern for field variables.
pub fn build_pattern(var_names: &[syn::Ident]) -> TokenStream {
    if var_names.is_empty() {
        return quote! { _ };
    }

    let mut iter = var_names.iter();
    let first = iter.next().unwrap();
    let mut pattern = quote! { #first };

    for ident in iter {
        pattern = quote! { (#pattern, #ident) };
    }

    pattern
}
