//! Parser chain building and AST constructor generation.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::field_kind::CollectedField;
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
        type_lattice: &syn::Path,
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
                        type_lattice,
                    )));
                }
            }
        }

        // Build the parser chain
        if parser_parts.is_empty() {
            return Ok(quote! { #crate_path::chumsky::prelude::empty() });
        }

        // Find the first field parser
        let first_field_idx = parser_parts
            .iter()
            .position(|p| matches!(p, ParserPart::Field(_)));

        let mut parser_expr: Option<TokenStream> = None;

        for (i, part) in parser_parts.iter().enumerate() {
            match part {
                ParserPart::Token(tok_parser) => {
                    match &parser_expr {
                        Some(expr) => {
                            parser_expr = Some(quote! { #expr.then_ignore(#tok_parser) });
                        }
                        None => {
                            // Check if there's a field coming after
                            if first_field_idx.is_some() && i < first_field_idx.unwrap() {
                                // Don't set parser_expr yet - we'll use ignore_then
                                continue;
                            } else {
                                // No fields, just use ignored()
                                parser_expr = Some(quote! { #tok_parser });
                            }
                        }
                    }
                }
                ParserPart::Field(field_parser) => {
                    match &parser_expr {
                        Some(expr) => {
                            parser_expr = Some(quote! { #expr.then(#field_parser) });
                        }
                        None => {
                            // Check if there are preceding tokens
                            let preceding_tokens: Vec<_> = parser_parts[..i]
                                .iter()
                                .filter_map(|p| match p {
                                    ParserPart::Token(t) => Some(t.clone()),
                                    _ => None,
                                })
                                .collect();

                            if !preceding_tokens.is_empty() {
                                // Combine preceding tokens
                                let mut combined = preceding_tokens[0].clone();
                                for tok in &preceding_tokens[1..] {
                                    combined = quote! { #combined.then_ignore(#tok) };
                                }
                                parser_expr = Some(quote! { #combined.ignore_then(#field_parser) });
                            } else {
                                parser_expr = Some(field_parser.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(parser_expr.unwrap_or_else(|| quote! { #crate_path::chumsky::prelude::empty() }))
    }

    /// Generate field parser based on field kind and format option.
    fn field_parser(
        &self,
        crate_path: &syn::Path,
        field: &CollectedField,
        opt: &FormatOption,
        ast_name: &syn::Ident,
        type_lattice: &syn::Path,
    ) -> TokenStream {
        let base = field
            .kind
            .parser_expr(crate_path, opt, ast_name, type_lattice);
        field.collection.wrap_parser(base)
    }

    /// Generate AST constructor that combines field occurrences.
    pub(super) fn ast_constructor(
        &self,
        ast_name: &syn::Ident,
        variant: Option<&syn::Ident>,
        collected: &[CollectedField],
        occurrences: &[FieldOccurrence<'_>],
        crate_path: &syn::Path,
    ) -> TokenStream {
        // Group occurrences by field index
        let mut field_occurrences: HashMap<usize, Vec<&FieldOccurrence>> = HashMap::new();
        for occ in occurrences {
            field_occurrences
                .entry(occ.field.index)
                .or_default()
                .push(occ);
        }

        // Filter to only fields that should be in the AST:
        // - Fields that are in the format string (have occurrences), OR
        // - Fields that don't have a default value
        let ast_fields: Vec<_> = collected
            .iter()
            .filter(|f| field_occurrences.contains_key(&f.index) || f.default.is_none())
            .collect();

        // Check if we have named fields
        let has_named = ast_fields.first().and_then(|f| f.ident.as_ref()).is_some();

        if has_named {
            let assigns = ast_fields.iter().map(|field| {
                let name = field.ident.as_ref().unwrap();
                let value = self.build_field_value(field, &field_occurrences, crate_path);
                quote! { #name: #value }
            });
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                // For named structs (not enum variants), add the _marker field
                None => quote! { #ast_name { #(#assigns,)* _marker: ::core::marker::PhantomData } },
            }
        } else {
            // For tuple fields, sort by original index to match AST struct definition order
            let mut sorted_ast_fields: Vec<_> = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let values = sorted_ast_fields
                .iter()
                .map(|field| self.build_field_value(field, &field_occurrences, crate_path));
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                // For tuple structs (not enum variants), add PhantomData at the end
                None => quote! { #ast_name ( #(#values,)* ::core::marker::PhantomData ) },
            }
        }
    }

    /// Build the value expression for a field based on its occurrences.
    fn build_field_value(
        &self,
        field: &CollectedField,
        field_occurrences: &HashMap<usize, Vec<&FieldOccurrence>>,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let occs = field_occurrences.get(&field.index);

        match occs {
            None => {
                // Field not in format string - this should be caught by validation in
                // validate_format, so this case is unreachable in practice.
                unreachable!(
                    "field '{}' not in format string - this should have been caught earlier",
                    field
                )
            }
            Some(occs) if occs.len() == 1 => {
                // Single occurrence - use the variable directly or wrap if needed
                let occ = occs[0];
                let var = &occ.var_name;

                match &occ.option {
                    // SSA/Result with only :name - need to create value with None type
                    FormatOption::Name => field
                        .kind
                        .construct_from_name_only(crate_path, var)
                        .unwrap_or_else(|| quote! { #var }),
                    // :type only should have been caught by validation
                    FormatOption::Type if field.kind.supports_name_type_options() => {
                        unreachable!(
                            "field '{}' has only :type occurrence - this should have been caught by validation",
                            field
                        )
                    }
                    // Default case - variable is already the correct type
                    _ => quote! { #var },
                }
            }
            Some(occs) => {
                // Multiple occurrences - need to combine them
                // Find :name and :type occurrences
                let name_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Name));
                let type_occ = occs.iter().find(|o| matches!(o.option, FormatOption::Type));

                match (name_occ, type_occ) {
                    // SSA/Result with both :name and :type
                    (Some(name), Some(ty)) => field
                        .kind
                        .construct_from_name_and_type(crate_path, &name.var_name, &ty.var_name)
                        .unwrap_or_else(|| {
                            let var = &occs[0].var_name;
                            quote! { #var }
                        }),
                    // Fallback - use the first occurrence
                    _ => {
                        let var = &occs[0].var_name;
                        quote! { #var }
                    }
                }
            }
        }
    }

    /// Generates a parser for a sequence of tokens.
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

    // Build nested tuple pattern for fields
    let mut iter = var_names.iter();
    let first = iter.next().unwrap();
    let mut pattern = quote! { #first };

    for ident in iter {
        pattern = quote! { (#pattern, #ident) };
    }

    pattern
}
