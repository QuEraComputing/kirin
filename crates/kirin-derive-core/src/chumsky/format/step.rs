use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;

use crate::derive::Compile;
use crate::gadgets::{CratePath, TraitPath};
use crate::ir::*;
use crate::kirin::extra::FieldKind;

use super::context::DeriveHasParser;
use super::parse::{Format, FormatElement};

pub struct ParseElements {
    crate_path: TokenStream,
    name: TokenStream,
    unspecified: BTreeMap<String, TokenStream>,
    results: Vec<String>,
    steps: Vec<ParseStep>,
    error: Option<syn::Error>,
}

impl ParseElements {
    pub fn new(
        crate_path: TokenStream,
        name: TokenStream,
        steps: Vec<ParseStep>,
        unspecified: BTreeMap<String, TokenStream>,
        results: Vec<String>,
    ) -> Self {
        Self {
            crate_path,
            name,
            steps,
            unspecified,
            results,
            error: None,
        }
    }

    pub fn error<T: std::fmt::Display>(msg: T) -> Self {
        Self {
            crate_path: quote! {},
            name: quote! {},
            steps: vec![],
            unspecified: BTreeMap::new(),
            results: Vec::new(),
            error: Some(syn::Error::new(Span::call_site(), msg)),
        }
    }
}

pub enum ParseStep {
    Ignore(TokenStream),
    Keep(TokenStream, TokenStream),
    Error(syn::Error),
}

impl ParseStep {
    pub fn error(msg: impl std::fmt::Display) -> Self {
        Self::Error(syn::Error::new(Span::call_site(), msg))
    }
}

impl<'a, 'src: 'a> Compile<'src, DeriveHasParser, ParseElements>
    for Fields<'a, 'src, DeriveHasParser>
{
    fn compile(&self, ctx: &DeriveHasParser) -> ParseElements {
        let crate_path: CratePath = self.compile(ctx);
        let trait_path: TraitPath = self.compile(ctx);
        let fields_map =
            BTreeMap::from_iter(self.iter().map(|f| (f.source_ident().to_string(), f)));
        let mut fields_specified = BTreeMap::new();

        let Some(format) = (match self.definition() {
            DefinitionStructOrVariant::Struct(data) => &data.attrs.format,
            DefinitionStructOrVariant::Variant(e, i) => &e.variants[*i].attrs.format,
        }) else {
            return ParseElements::error("No format specified");
        };

        let Ok(format) = Format::parse(format, None) else {
            return ParseElements::error("Failed to parse format");
        };

        let steps: Vec<ParseStep> = format
            .elements()
            .iter()
            .map(|elem| match elem {
                FormatElement::Token(tokens) => {
                    let mut iter = tokens.iter();
                    let Some(first_token) = iter.next() else {
                        let msg = "Format string cannot be empty";
                        return ParseStep::error(msg);
                    };
                    let mut expr = quote! { just(#crate_path::#first_token) };
                    for t in iter {
                        expr = quote! { #expr.then(just(#crate_path::#t)) };
                    }
                    ParseStep::Ignore(expr)
                }
                FormatElement::Field(name, _opt) => {
                    let Some(f) = fields_map.get(*name) else {
                        return ParseStep::error(format!("Field '{}' not found for format", name));
                    };

                    fields_specified.insert(name.to_string(), f);
                    let ty = &f.source().ty;
                    let expr = match f.extra().kind {
                        FieldKind::SSAValue => quote! { #crate_path::operand() },
                        FieldKind::Block => quote! { #crate_path::block() },
                        FieldKind::Successor => quote! { #crate_path::successor() },
                        FieldKind::Region => quote! { #crate_path::region() },
                        FieldKind::Other => quote! {
                            <#ty as #trait_path<'tokens, 'src, _AnotherLanguage>>::parser()
                        },
                        _ => {
                            return ParseStep::error(format!(
                                "Field '{}' ({}) cannot be used in format",
                                name,
                                f.extra().kind
                            ));
                        }
                    };
                    ParseStep::Keep(expr, quote! { #f })
                }
            })
            .collect();

        let name = format_ident!("{}SyntaxTree", self.source_ident());
        let unspecified: BTreeMap<String, TokenStream> = fields_map
            .iter()
            .filter_map(|(name, f)| {
                if fields_specified.contains_key(name)
                    || matches!(f.extra().kind, FieldKind::ResultValue)
                {
                    None
                } else {
                    Some((name.clone(), quote! { #f: Default::default() }))
                }
            })
            .collect();
        let results: Vec<String> = self
            .iter()
            .filter_map(|f| {
                if matches!(f.extra().kind, FieldKind::ResultValue) {
                    Some(f.source_ident().to_string())
                } else {
                    None
                }
            })
            .collect();

        ParseElements::new(
            quote! {#crate_path},
            quote! {#name},
            steps,
            unspecified,
            results,
        )
    }
}

impl ToTokens for ParseElements {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(err) = &self.error {
            err.to_compile_error().to_tokens(tokens);
            return;
        }

        let name = &self.name;
        let crate_path = &self.crate_path;
        let mut iter = self.steps.iter();

        let (mut chain, mut pattern, mut vars, mut is_keeping) = match iter.next() {
            Some(ParseStep::Keep(token, var)) => {
                (token.clone(), var.clone(), vec![var.clone()], true)
            }
            Some(ParseStep::Ignore(token)) => (token.clone(), quote! { _ }, vec![], false),
            Some(ParseStep::Error(e)) => {
                e.to_compile_error().to_tokens(tokens);
                return;
            }
            None => {
                return quote! {
                    #crate_path::empty().to(#name)
                }
                .to_tokens(tokens);
            }
        };

        for step in iter {
            match step {
                ParseStep::Keep(token, var) => {
                    let var = var.clone();
                    if is_keeping {
                        chain = quote! { #chain.then(#token) };
                        pattern = quote! { (#pattern, #var) };
                    } else {
                        chain = quote! { #chain.ignore_then(#token) };
                        pattern = quote! { #var };
                        is_keeping = true;
                    }
                    vars.push(var);
                }
                ParseStep::Ignore(token) => {
                    if is_keeping {
                        chain = quote! { #chain.then_ignore(#token) };
                    } else {
                        chain = quote! { #chain.ignore_then(#token) };
                    }
                }
                ParseStep::Error(e) => {
                    e.to_compile_error().to_tokens(tokens);
                    return;
                }
            }
        }

        for unspecified in self.unspecified.values() {
            vars.push(unspecified.clone());
        }

        let n_result = self.results.len();
        for (idx, result) in self.results.iter().enumerate() {
            let name = format_ident!("{}", result);
            vars.push(quote! { #name: chumsky_parsed_results[#idx].clone() })
        }

        quote! {
            #crate_path::result_values(#n_result).then(#chain).map(move |(chumsky_parsed_results, #pattern)| {
                #name {
                    #( #vars ),*
                }
            })
        }
        .to_tokens(tokens);
    }
}
