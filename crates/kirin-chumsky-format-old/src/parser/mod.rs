use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    ast::{
        build_format_usage, CollectedField, DeriveChumskyAst, FieldCollector, FormatUsage,
        SyntaxFieldKind,
    },
    parse::{Format, FormatElement, FormatOption},
    ChumskyLayout,
};

pub struct DeriveChumskyParser {
    default_crate_path: syn::Path,
}

impl DeriveChumskyParser {
    pub fn new(ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>) -> Self {
        let default_crate_path: syn::Path = ir_input
            .extra_attrs
            .crate_path
            .as_ref()
            .or(ir_input.attrs.crate_path.as_ref())
            .cloned()
            .unwrap_or_else(|| syn::parse_quote!(::kirin_chumsky_2));
        Self { default_crate_path }
    }

    pub fn generate(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        let ast_deriver = DeriveChumskyAst::new(ir_input);
        let ast_tokens = ast_deriver.generate(ir_input);
        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());
        let mut ast_generics = ir_input.generics.clone();

        let tokens_lt = syn::Lifetime::new("'tokens", proc_macro2::Span::call_site());
        if !ast_generics.params.iter().any(|p| {
            matches!(
                p,
                syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"
            )
        }) {
            ast_generics
                .params
                .insert(0, syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())));
        }

        let src_lt = syn::Lifetime::new("'src", proc_macro2::Span::call_site());
        if !ast_generics.params.iter().any(|p| {
            matches!(
                p,
                syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"
            )
        }) {
            let mut src_param = syn::LifetimeParam::new(src_lt.clone());
            src_param.bounds.push(tokens_lt.clone());
            ast_generics
                .params
                .insert(1, syn::GenericParam::Lifetime(src_param));
        }

        let lang_ident = syn::Ident::new("Language", proc_macro2::Span::call_site());
        if !ast_generics.params.iter().any(|p| {
            matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident)
        }) {
            let mut lang_param = syn::TypeParam::from(lang_ident.clone());
            let crate_path = self.resolve_crate_path(ir_input, None);
            lang_param
                .bounds
                .push(syn::parse_quote!(#crate_path::LanguageChumskyParser<'tokens, 'src>));
            ast_generics
                .params
                .push(syn::GenericParam::Type(lang_param));
        }

        let crate_path = self.resolve_crate_path(ir_input, None);
        let parser_impl =
            self.generate_parser_impl(ir_input, &ast_name, &ast_generics, &crate_path);

        quote! {
            #ast_tokens
            #parser_impl
        }
    }

    fn generate_parser_impl(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(s) => {
                self.generate_struct_parser(ir_input, &s.0, ast_name, ast_generics, crate_path)
            }
            kirin_derive_core_2::ir::Data::Enum(e) => {
                self.generate_enum_parser(ir_input, e, ast_name, ast_generics, crate_path)
            }
        }
    }

    fn generate_struct_parser(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = ast_generics.split_for_impl();
        let ast_where = ir_input.generics.where_clause.clone();
        let parser_body = self
            .build_statement_parser(ir_input, stmt, ast_name, ast_generics, None, crate_path)
            .expect("failed to build parser");

        quote! {
            impl #impl_generics #crate_path::WithRecursiveChumskyParser<'tokens, 'src, Language>
                for #ast_name #ty_generics #where_clause
                #ast_where
            {
                type Output = Self;
                fn recursive<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, Language::Output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    #parser_body.boxed()
                }
            }
        }
    }

    fn generate_enum_parser(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core_2::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = ast_generics.split_for_impl();
        let ast_where = ir_input.generics.where_clause.clone();

        let mut variant_parsers = Vec::new();
        for variant in &data.variants {
            let parser = self.build_statement_parser(
                ir_input,
                variant,
                ast_name,
                ast_generics,
                Some(&variant.name),
                crate_path,
            );
            match parser {
                Ok(p) => variant_parsers.push(p),
                Err(err) => variant_parsers.push(err.to_compile_error()),
            }
        }

        let combined = variant_parsers
            .into_iter()
            .reduce(|acc, parser| quote! { #acc.or(#parser) })
            .expect("at least one variant parser");

        quote! {
            impl #impl_generics #crate_path::WithRecursiveChumskyParser<'tokens, 'src, Language>
                for #ast_name #ty_generics #where_clause
                #ast_where
            {
                type Output = Self;
                fn recursive<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, Language::Output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    #combined.boxed()
                }
            }
        }
    }

    fn build_statement_parser(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        variant: Option<&syn::Ident>,
        crate_path: &syn::Path,
    ) -> syn::Result<TokenStream> {
        let format_str = self
            .format_for_statement(ir_input, stmt)
            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing chumsky format"))?;
        let format = Format::parse(&format_str, None)?;

        let format_usage = build_format_usage(stmt, &format);
        let collected = collect_fields(stmt, crate_path, format_usage.clone());
        let field_sequence =
            field_sequence_from_format(stmt, &format, &collected).ok_or_else(|| {
                syn::Error::new(stmt.name.span(), "invalid format field ordering")
            })?;

        let field_idents: Vec<_> = field_sequence
            .iter()
            .enumerate()
            .map(|(i, f)| {
                f.ident
                    .clone()
                    .unwrap_or_else(|| syn::Ident::new(&format!("field_{i}"), proc_macro2::Span::call_site()))
            })
            .collect();
        let mut field_iter = field_sequence.iter();

        let mut parser_expr: Option<TokenStream> = None;
        for elem in format.elements() {
            match elem {
                FormatElement::Token(tokens) => {
                    let tok_parser = token_parser(tokens);
                    parser_expr = Some(match parser_expr {
                        Some(expr) => quote! { #expr.then_ignore(#tok_parser) },
                        None => quote! { #tok_parser },
                    });
                }
                FormatElement::Field(_, opt) => {
                    let next_field = field_iter
                        .next()
                        .ok_or_else(|| syn::Error::new(stmt.name.span(), "unexpected format field"))?;
                    let parser =
                        field_parser(crate_path, next_field, opt)
                            .ok_or_else(|| syn::Error::new(stmt.name.span(), "missing field parser"))?;
                    parser_expr = Some(match parser_expr {
                        Some(expr) => quote! { #expr.then(#parser) },
                        None => parser,
                    });
                }
            }
        }

        let parser_expr = parser_expr.ok_or_else(|| syn::Error::new(stmt.name.span(), "empty format"))?;
        let pattern = nested_pattern(&field_idents);
        let constructor =
            ast_constructor(ast_name, ast_generics, variant, &field_idents, &field_sequence);

        Ok(quote! {{
            use ::kirin_lexer::Token;
            #parser_expr.map(|#pattern| #constructor)
        }})
    }

    fn resolve_crate_path(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        stmt: Option<&kirin_derive_core_2::ir::Statement<ChumskyLayout>>,
    ) -> syn::Path {
        stmt.and_then(|s| s.extra_attrs.crate_path.clone())
            .or(ir_input.extra_attrs.crate_path.clone())
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| self.default_crate_path.clone())
    }

    fn format_for_statement(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
    ) -> Option<String> {
        stmt.extra_attrs
            .format
            .clone()
            .or(stmt.attrs.format.clone())
            .or(ir_input.extra_attrs.format.clone())
    }
}

fn token_parser(tokens: &[kirin_lexer::Token<'_>]) -> TokenStream {
    let mut iter = tokens.iter();
    let Some(first) = iter.next() else {
        return quote! { chumsky::prelude::empty().ignored() };
    };
    let mut parser = quote! { chumsky::prelude::just(#first) };
    for tok in iter {
        parser = quote! { #parser.then_ignore(chumsky::prelude::just(#tok)) };
    }
    quote! { #parser.ignored() }
}

fn field_parser(
    crate_path: &syn::Path,
    field: &CollectedField,
    opt: &FormatOption,
) -> Option<TokenStream> {
    let base = match field.syntax.kind {
        SyntaxFieldKind::SSAValue => match opt {
            FormatOption::Name => quote! { #crate_path::parsers::nameof_ssa() },
            FormatOption::Type => quote! { #crate_path::parsers::typeof_ssa() },
            FormatOption::Default => quote! { #crate_path::parsers::ssa() },
        },
        SyntaxFieldKind::ResultValue => match opt {
            FormatOption::Type => quote! { #crate_path::parsers::typeof_ssa() },
            _ => quote! { #crate_path::parsers::result_value() },
        },
        SyntaxFieldKind::NameofSSAValue => quote! { #crate_path::parsers::nameof_ssa() },
        SyntaxFieldKind::TypeofSSAValue | SyntaxFieldKind::TypeofResultValue => {
            quote! { #crate_path::parsers::typeof_ssa() }
        }
        SyntaxFieldKind::Block => quote! { #crate_path::parsers::block(language.clone()) },
        SyntaxFieldKind::Successor => quote! { #crate_path::parsers::block(language.clone()) },
        SyntaxFieldKind::Region => quote! { #crate_path::parsers::region(language.clone()) },
        SyntaxFieldKind::AsIs => {
            let ty = &field.syntax.ty;
            quote! { <#ty as #crate_path::WithChumskyParser<'tokens, 'src>>::parser() }
        }
    };

    let parser = match field.collection {
        kirin_derive_core_2::ir::fields::Collection::Single => base,
        kirin_derive_core_2::ir::fields::Collection::Vec => quote! { #base.repeated().collect() },
        kirin_derive_core_2::ir::fields::Collection::Option => quote! { #base.or_not() },
    };
    Some(parser)
}

fn field_sequence_from_format<'a>(
    stmt: &'a kirin_derive_core_2::ir::Statement<ChumskyLayout>,
    format: &Format<'_>,
    collected: &'a [CollectedField],
) -> Option<Vec<&'a CollectedField>> {
    let mut map_by_ident = HashMap::new();
    for arg in stmt.arguments.iter() {
        if let Some(ident) = &arg.field.ident {
            map_by_ident.insert(ident.to_string(), arg.field.index);
        }
    }
    for res in stmt.results.iter() {
        if let Some(ident) = &res.field.ident {
            map_by_ident.insert(ident.to_string(), res.field.index);
        }
    }
    for b in stmt.blocks.iter() {
        if let Some(ident) = &b.field.ident {
            map_by_ident.insert(ident.to_string(), b.field.index);
        }
    }
    for r in stmt.regions.iter() {
        if let Some(ident) = &r.field.ident {
            map_by_ident.insert(ident.to_string(), r.field.index);
        }
    }
    for s in stmt.successors.iter() {
        if let Some(ident) = &s.field.ident {
            map_by_ident.insert(ident.to_string(), s.field.index);
        }
    }
    for v in stmt.values.iter() {
        if let Some(ident) = &v.field.ident {
            map_by_ident.insert(ident.to_string(), v.field.index);
        }
    }

    let mut occurrences: HashMap<usize, usize> = HashMap::new();
    let mut seq = Vec::new();
    for elem in format.elements() {
        if let FormatElement::Field(name, _) = elem {
            let key = name.to_string();
            let index = name
                .parse::<usize>()
                .ok()
                .or_else(|| map_by_ident.get(&key).copied())?;
            let occ = occurrences.entry(index).or_default();
            let field = collected
                .iter()
                .find(|f| f.index == index && f.occurrence == *occ)?;
            *occ += 1;
            seq.push(field);
        }
    }
    Some(seq)
}

fn nested_pattern(idents: &[syn::Ident]) -> TokenStream {
    if idents.is_empty() {
        return quote! { () };
    }
    let mut iter = idents.iter();
    let mut pattern: TokenStream = {
        let first = iter.next().unwrap();
        quote! { #first }
    };
    for ident in iter {
        pattern = quote! { (#pattern, #ident) };
    }
    pattern
}

fn ast_constructor(
    ast_name: &syn::Ident,
    ast_generics: &syn::Generics,
    variant: Option<&syn::Ident>,
    field_idents: &[syn::Ident],
    collected: &[&CollectedField],
) -> TokenStream {
    let (_, ty_generics, _) = ast_generics.split_for_impl();
    let turbofish = quote! { ::#ty_generics };
    if collected.first().and_then(|f| f.ident.as_ref()).is_some() {
        let assigns = collected.iter().zip(field_idents).map(|(field, ident)| {
            let name = field.ident.as_ref().unwrap_or(ident);
            quote! { #name: #ident }
        });
        match variant {
            Some(v) => quote! { #ast_name #turbofish :: #v { #(#assigns),* } },
            None => quote! { #ast_name #turbofish { #(#assigns),* } },
        }
    } else {
        match variant {
            Some(v) => quote! { #ast_name #turbofish :: #v ( #(#field_idents),* ) },
            None => quote! { #ast_name #turbofish ( #(#field_idents),* ) },
        }
    }
}

fn collect_fields(
    stmt: &kirin_derive_core_2::ir::Statement<ChumskyLayout>,
    crate_path: &syn::Path,
    format_usage: FormatUsage,
) -> Vec<CollectedField> {
    let mut collector = FieldCollector::new(crate_path.clone(), format_usage);
    kirin_derive_core_2::scan::scan_statement(&mut collector, stmt)
        .expect("failed to collect syntax fields");
    collector.finish()
}

#[cfg(test)]
mod tests;
