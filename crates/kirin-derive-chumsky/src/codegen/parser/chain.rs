//! Parser chain building and AST constructor generation.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use kirin_derive_toolkit::ir::fields::{Collection, FieldInfo};

use kirin_derive_toolkit::ir::fields::FieldCategory;

use crate::field_kind;
use crate::format::{BodyProjection, Format, FormatElement, FormatOption};
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
                FormatElement::Keyword(name) => {
                    parser_parts.push(ParserPart::Token(self.keyword_parser(name)));
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
                FormatElement::Context(proj) => {
                    // Context projections are parsed and discarded at the
                    // statement level — the function text parser provides the values.
                    match proj {
                        crate::format::ContextProjection::Name => {
                            // Parse and discard the @symbol — the function text parser
                            // extracts the name from EmitContext after parse_and_emit.
                            parser_parts.push(ParserPart::Token(
                                quote! { #crate_path::symbol() },
                            ));
                        }
                    }
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
        let base = field_kind::parser_expr(field, crate_path, opt, ast_name, ir_type, type_params);
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
            quote! { ::core::marker::PhantomData::<fn() -> (&'t (), __TypeOutput, __LanguageOutput)> }
        } else {
            quote! { ::core::marker::PhantomData::<fn() -> (&'t (), #(#type_params,)* __TypeOutput, __LanguageOutput)> }
        };

        // Track result index for multi-result support
        let mut result_idx = 0usize;

        if has_named {
            let assigns: Vec<_> = ast_fields
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().unwrap();
                    let ri = if field.category() == FieldCategory::Result {
                        let idx = result_idx;
                        result_idx += 1;
                        Some(idx)
                    } else {
                        None
                    };
                    let value =
                        self.build_field_value(field, &field_occurrences, crate_path, ri);
                    quote! { #name: #value }
                })
                .collect();
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                None => quote! { #ast_name { #(#assigns,)* _marker: #phantom_data } },
            }
        } else {
            let mut sorted_ast_fields: Vec<_> = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let values: Vec<_> = sorted_ast_fields
                .iter()
                .map(|field| {
                    let ri = if field.category() == FieldCategory::Result {
                        let idx = result_idx;
                        result_idx += 1;
                        Some(idx)
                    } else {
                        None
                    };
                    self.build_field_value(field, &field_occurrences, crate_path, ri)
                })
                .collect();
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                None => quote! { #ast_name ( #(#values,)* #phantom_data ) },
            }
        }
    }

    /// Builds an AST constructor for new-format mode where result names
    /// come from the generic `result_name_list()` parser.
    ///
    /// The `__result_names` variable is a `Vec<Spanned<&'t str>>` available
    /// in the generated code, containing the parsed result names in order.
    pub(super) fn ast_constructor_new_format(
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
            .filter(|f| {
                // In new-format, result fields are always included (even without occurrences)
                f.category() == FieldCategory::Result
                    || field_occurrences.contains_key(&f.index)
                    || !f.has_default()
            })
            .collect();

        let has_named = ast_fields.first().and_then(|f| f.ident.as_ref()).is_some();

        let phantom_data = if type_params.is_empty() {
            quote! { ::core::marker::PhantomData::<fn() -> (&'t (), __TypeOutput, __LanguageOutput)> }
        } else {
            quote! { ::core::marker::PhantomData::<fn() -> (&'t (), #(#type_params,)* __TypeOutput, __LanguageOutput)> }
        };

        // Count result fields to generate index-based access into __result_names
        let mut result_idx = 0usize;

        if has_named {
            let assigns: Vec<_> = ast_fields
                .iter()
                .map(|field| {
                    let name = field.ident.as_ref().unwrap();
                    if field.category() == FieldCategory::Result {
                        let value = self.build_new_format_result_value(
                            field,
                            &field_occurrences,
                            crate_path,
                            result_idx,
                        );
                        result_idx += 1;
                        quote! { #name: #value }
                    } else {
                        let value =
                            self.build_field_value(field, &field_occurrences, crate_path, None);
                        quote! { #name: #value }
                    }
                })
                .collect();
            match variant {
                Some(v) => quote! { #ast_name::#v { #(#assigns),* } },
                None => quote! { #ast_name { #(#assigns,)* _marker: #phantom_data } },
            }
        } else {
            let mut sorted_ast_fields: Vec<_> = ast_fields.clone();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let values: Vec<_> = sorted_ast_fields
                .iter()
                .map(|field| {
                    if field.category() == FieldCategory::Result {
                        let value = self.build_new_format_result_value(
                            field,
                            &field_occurrences,
                            crate_path,
                            result_idx,
                        );
                        result_idx += 1;
                        value
                    } else {
                        self.build_field_value(field, &field_occurrences, crate_path, None)
                    }
                })
                .collect();
            match variant {
                Some(v) => quote! { #ast_name::#v ( #(#values),* ) },
                None => quote! { #ast_name ( #(#values,)* #phantom_data ) },
            }
        }
    }

    /// Builds a ResultValue AST field for new-format mode.
    ///
    /// The name comes from `__result_names[idx]` and the type comes from
    /// the `:type` occurrence in the format string (if present).
    fn build_new_format_result_value(
        &self,
        field: &FieldInfo<ChumskyLayout>,
        field_occurrences: &HashMap<usize, Vec<&FieldOccurrence>>,
        crate_path: &syn::Path,
        result_idx: usize,
    ) -> TokenStream {
        let type_occ = field_occurrences
            .get(&field.index)
            .and_then(|occs| occs.iter().find(|o| matches!(o.option, FormatOption::Type)));

        let ty_expr = if let Some(type_occ) = type_occ {
            let var = &type_occ.var_name;
            quote! { Some(#var.ty.clone()) }
        } else {
            quote! { None }
        };

        quote! {
            #crate_path::ResultValue {
                name: __result_names[#result_idx].clone(),
                ty: #ty_expr,
                result_index: #result_idx,
            }
        }
    }

    fn build_field_value(
        &self,
        field: &FieldInfo<ChumskyLayout>,
        field_occurrences: &HashMap<usize, Vec<&FieldOccurrence>>,
        crate_path: &syn::Path,
        result_index: Option<usize>,
    ) -> TokenStream {
        let occs = field_occurrences.get(&field.index);
        match occs {
            None => {
                unreachable!(
                    "field '{}' not in format string - this should have been caught earlier",
                    field
                )
            }
            // Check for body projection occurrences — reconstruct flat AST from pieces
            Some(occs)
                if occs
                    .iter()
                    .any(|o| matches!(o.option, FormatOption::Body(_))) =>
            {
                self.build_projected_field_value(field, occs, crate_path)
            }
            // Check for signature projection occurrences — reconstruct Signature from pieces
            Some(occs)
                if occs
                    .iter()
                    .any(|o| matches!(o.option, FormatOption::Signature(_))) =>
            {
                self.build_projected_signature_value(occs)
            }
            Some(occs) if occs.len() == 1 => {
                let occ = occs[0];
                let var = &occ.var_name;

                match &occ.option {
                    FormatOption::Name => {
                        field_kind::construct_from_name_only(
                            field,
                            crate_path,
                            var,
                            result_index,
                        )
                        .unwrap_or_else(|| quote! { #var })
                    }
                    FormatOption::Type if field.category().is_ssa_like() => {
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
                    (Some(name), Some(ty)) => field_kind::construct_from_name_and_type(
                        field,
                        crate_path,
                        &name.var_name,
                        &ty.var_name,
                        result_index,
                    )
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

    /// Reconstructs a flat AST type from individually-parsed projection pieces.
    fn build_projected_field_value(
        &self,
        field: &FieldInfo<ChumskyLayout>,
        occs: &[&FieldOccurrence],
        crate_path: &syn::Path,
    ) -> TokenStream {
        // Helper: find the variable for a specific body projection
        let find_var = |proj: BodyProjection| -> Option<&syn::Ident> {
            occs.iter()
                .find(|o| matches!(&o.option, FormatOption::Body(p) if *p == proj))
                .map(|o| &o.var_name)
        };

        match field.category() {
            FieldCategory::DiGraph => {
                let ports_expr = find_var(BodyProjection::Ports)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
                let captures_expr = find_var(BodyProjection::Captures)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
                // digraph_body_statements returns (Vec<Spanned<S>>, Vec<Spanned<&str>>)
                let (stmts_expr, yields_expr) = if let Some(body) = find_var(BodyProjection::Body)
                {
                    (quote! { #body.0 }, quote! { #body.1 })
                } else {
                    (
                        quote! { ::std::vec::Vec::new() },
                        quote! { ::std::vec::Vec::new() },
                    )
                };

                quote! {
                    #crate_path::DiGraph {
                        name: ::core::option::Option::None,
                        ports: #ports_expr,
                        captures: #captures_expr,
                        statements: #stmts_expr,
                        yields: #yields_expr,
                    }
                }
            }
            FieldCategory::UnGraph => {
                let ports_expr = find_var(BodyProjection::Ports)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
                let captures_expr = find_var(BodyProjection::Captures)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
                let stmts_expr = find_var(BodyProjection::Body)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });

                quote! {
                    #crate_path::UnGraph {
                        name: ::core::option::Option::None,
                        ports: #ports_expr,
                        captures: #captures_expr,
                        statements: #stmts_expr,
                    }
                }
            }
            FieldCategory::Block => {
                let args_expr = find_var(BodyProjection::Args)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });
                let stmts_expr = find_var(BodyProjection::Body)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });

                // Block fields are Spanned<Block>, so we wrap in Spanned
                quote! {
                    #crate_path::Spanned {
                        value: #crate_path::Block {
                            label: ::core::option::Option::None,
                            arguments: #args_expr,
                            statements: #stmts_expr,
                        },
                        span: #crate_path::chumsky::span::SimpleSpan::from(0..0),
                    }
                }
            }
            FieldCategory::Region => {
                // region_body returns Vec<Spanned<Block>>
                let blocks_expr = find_var(BodyProjection::Body)
                    .map(|v| quote! { #v })
                    .unwrap_or_else(|| quote! { ::std::vec::Vec::new() });

                quote! {
                    #crate_path::Region {
                        blocks: #blocks_expr,
                    }
                }
            }
            _ => unreachable!("body projections only valid on body field types"),
        }
    }

    /// Reconstruct a `Signature` from its `{sig:inputs}` and `{sig:return}` projection variables.
    fn build_projected_signature_value(&self, occs: &[&FieldOccurrence]) -> TokenStream {
        use crate::format::SignatureProjection;
        let ir_path = &self.config.ir_path;
        let find_sig_var = |proj: SignatureProjection| -> Option<&syn::Ident> {
            occs.iter()
                .find(|o| matches!(&o.option, FormatOption::Signature(p) if *p == proj))
                .map(|o| &o.var_name)
        };
        let inputs_var = find_sig_var(SignatureProjection::Inputs)
            .expect("validation ensures {sig:inputs} is present");
        let return_var = find_sig_var(SignatureProjection::Return)
            .expect("validation ensures {sig:return} is present");
        quote! { #ir_path::Signature::new(#inputs_var, #return_var, ()) }
    }

    pub(super) fn token_parser(&self, tokens: &[kirin_lexer::Token<'_>]) -> TokenStream {
        use kirin_lexer::Token as T;
        let crate_path = &self.config.crate_path;

        // Map format-string escape tokens to their runtime equivalents.
        // In the format string, {{ produces EscapedLBrace, but at runtime
        // the input lexer produces LBrace for literal {.
        let map_token = |tok: &kirin_lexer::Token<'_>| -> TokenStream {
            match tok {
                T::EscapedLBrace => quote! { #crate_path::Token::LBrace },
                T::EscapedRBrace => quote! { #crate_path::Token::RBrace },
                other => quote! { #other },
            }
        };

        let mut iter = tokens.iter();
        let Some(first) = iter.next() else {
            return quote! { #crate_path::chumsky::prelude::empty().ignored() };
        };
        let first_tok = map_token(first);
        let mut parser = quote! { #crate_path::chumsky::prelude::just(#first_tok) };
        for tok in iter {
            let mapped = map_token(tok);
            parser = quote! { #parser.then_ignore(#crate_path::chumsky::prelude::just(#mapped)) };
        }
        parser
    }

    fn keyword_parser(&self, name: &str) -> TokenStream {
        let crate_path = &self.config.crate_path;
        quote! {
            {
                let __keyword_parser: #crate_path::BoxedParser<'t, I, _> = if namespace.is_empty() {
                    #crate_path::chumsky::prelude::just(#crate_path::Token::Identifier(#name)).boxed()
                } else {
                    let mut __parts: ::std::vec::Vec<&'static str> = namespace.to_vec();
                    __parts.push(#name);
                    let mut __p: #crate_path::BoxedParser<'t, I, _> = #crate_path::chumsky::prelude::just(
                        #crate_path::Token::Identifier(__parts[0])
                    ).boxed();
                    for &__part in &__parts[1..] {
                        __p = __p
                            .then_ignore(#crate_path::chumsky::prelude::just(#crate_path::Token::Dot))
                            .then_ignore(#crate_path::chumsky::prelude::just(
                                #crate_path::Token::Identifier(__part)
                            ))
                            .boxed();
                    }
                    __p
                };
                __keyword_parser
            }
        }
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
