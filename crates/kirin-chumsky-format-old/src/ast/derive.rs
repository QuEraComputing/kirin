use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use super::{build_format_usage, CollectedField, FieldCollector, FormatUsage};

pub struct DeriveChumskyAst {
    default_crate_path: syn::Path,
}

impl DeriveChumskyAst {
    pub fn new(ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>) -> Self {
        let default_crate_path: syn::Path = ir_input
            .extra_attrs
            .crate_path
            .as_ref()
            .or(ir_input.attrs.crate_path.as_ref())
            .map(|p| parse_quote!(#p))
            .unwrap_or(parse_quote!(::kirin_chumsky_2));
        Self { default_crate_path }
    }

    pub fn generate(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
    ) -> TokenStream {
        let ir_name = &ir_input.name;
        let ast_name = syn::Ident::new(&format!("{}AST", ir_name), ir_name.span());
        let ast_type_def = self.generate_ast_type(ir_input, &ast_name);
        quote! { #ast_type_def }
    }

    fn generate_ast_type(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let mut ast_generics = ir_input.generics.clone();
        let crate_path = self.resolve_crate_path(ir_input, None);

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
            lang_param
                .bounds
                .push(syn::parse_quote!(#crate_path::LanguageChumskyParser<'tokens, 'src>));
            ast_generics
                .params
                .push(syn::GenericParam::Type(lang_param));
        }

        let where_clause = ir_input.generics.where_clause.clone();
        let (impl_generics, _ty_generics, _) = ast_generics.split_for_impl();

        let item = match &ir_input.data {
            kirin_derive_core_2::ir::Data::Struct(s) => {
                self.render_struct(ir_input, &s.0, ast_name, impl_generics, where_clause)
            }
            kirin_derive_core_2::ir::Data::Enum(e) => {
                self.render_enum(ir_input, e, ast_name, impl_generics, where_clause)
            }
        };

        quote! {
            #[allow(non_camel_case_types)]
            #[derive(Clone, Debug, PartialEq)]
            #item
        }
    }

    fn render_struct(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
        stmt: &kirin_derive_core_2::ir::Statement<crate::ChumskyLayout>,
        ast_name: &syn::Ident,
        impl_generics: syn::ImplGenerics<'_>,
        where_clause: Option<syn::WhereClause>,
    ) -> TokenStream {
        let format_usage = match self.format_usage_for_statement(ir_input, stmt) {
            Ok(usage) => usage,
            Err(err) => return err.to_compile_error(),
        };
        let fields =
            collect_fields(stmt, &self.resolve_crate_path(ir_input, Some(stmt)), format_usage);
        let body = render_fields(&fields, true);
        quote! { pub struct #ast_name #impl_generics #where_clause #body }
    }

    fn render_enum(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
        data: &kirin_derive_core_2::ir::DataEnum<crate::ChumskyLayout>,
        ast_name: &syn::Ident,
        impl_generics: syn::ImplGenerics<'_>,
        where_clause: Option<syn::WhereClause>,
    ) -> TokenStream {
        let mut errors: Vec<syn::Error> = Vec::new();
        let variants: Vec<_> = data
            .variants
            .iter()
            .map(|variant| {
                let variant_name = &variant.name;
                match self.format_usage_for_statement(ir_input, variant) {
                    Ok(format_usage) => {
                        let fields = collect_fields(
                            variant,
                            &self.resolve_crate_path(ir_input, Some(variant)),
                            format_usage,
                        );
                        let body = render_fields(&fields, false);
                        quote! { #variant_name #body }
                    }
                    Err(err) => {
                        errors.push(err);
                        quote! {}
                    }
                }
            })
            .collect();
        if !errors.is_empty() {
            let compile_errors = errors.iter().map(syn::Error::to_compile_error);
            return quote! { #(#compile_errors)* };
        }
        quote! { pub enum #ast_name #impl_generics #where_clause { #(#variants),* } }
    }

    fn format_usage_for_statement(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
        stmt: &kirin_derive_core_2::ir::Statement<crate::ChumskyLayout>,
    ) -> Result<FormatUsage, syn::Error> {
        let format = stmt
            .extra_attrs
            .format
            .clone()
            .or_else(|| stmt.attrs.format.clone())
            .or_else(|| ir_input.extra_attrs.format.clone())
            .ok_or_else(|| {
                syn::Error::new(
                    stmt.name.span(),
                    "chumsky format specification is required for this statement",
                )
            })?;

        let parsed = crate::parse::Format::parse(&format, None).map_err(|err| err)?;
        Ok(build_format_usage(stmt, &parsed))
    }

    fn resolve_crate_path(
        &self,
        ir_input: &kirin_derive_core_2::ir::Input<crate::ChumskyLayout>,
        stmt: Option<&kirin_derive_core_2::ir::Statement<crate::ChumskyLayout>>,
    ) -> syn::Path {
        stmt.and_then(|s| s.extra_attrs.crate_path.clone())
            .or(ir_input.extra_attrs.crate_path.clone())
            .or(ir_input.attrs.crate_path.clone())
            .unwrap_or_else(|| self.default_crate_path.clone())
    }
}

fn collect_fields(
    stmt: &kirin_derive_core_2::ir::Statement<crate::ChumskyLayout>,
    crate_path: &syn::Path,
    format_usage: FormatUsage,
) -> Vec<CollectedField> {
    let mut collector = FieldCollector::new(crate_path.clone(), format_usage);
    kirin_derive_core_2::scan::scan_statement(&mut collector, stmt)
        .expect("failed to collect syntax fields");
    collector.finish()
}

fn render_fields(fields: &[CollectedField], is_struct: bool) -> TokenStream {
    if fields.is_empty() {
        return if is_struct { quote! { ; } } else { quote! {} };
    }
    let is_named = fields[0].ident.is_some();
    if is_named {
        let rendered = fields.iter().map(|field| {
            let name = &field.ident;
            let ty = field.render_type();
            quote! { #name: #ty }
        });
        quote! {{ #(#rendered),* }}
    } else {
        let rendered = fields.iter().map(|field| field.render_type());
        if is_struct {
            quote! { ( #(#rendered),* ); }
        } else {
            quote! { ( #(#rendered),* ) }
        }
    }
}
