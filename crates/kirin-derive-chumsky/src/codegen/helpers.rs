use std::collections::HashSet;

use kirin_derive_toolkit::ir::fields::FieldInfo;
use kirin_derive_toolkit::ir::{Layout, VariantRef};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::ChumskyStatementAttrs;
use crate::field_kind::{collect_value_types_needing_bounds, fields_in_format};
use crate::format::Format;

/// Builds AST generics shared by AST and EmitIR generators.
pub(crate) fn build_ast_generics(
    base_generics: &syn::Generics,
    include_language: bool,
) -> syn::Generics {
    use proc_macro2::Span;

    let mut generics = base_generics.clone();

    let t_lt = syn::Lifetime::new("'t", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "t"))
    {
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(t_lt)),
        );
    }

    let type_output_ident = syn::Ident::new("TypeOutput", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == type_output_ident))
    {
        generics
            .params
            .push(syn::GenericParam::Type(syn::TypeParam::from(
                type_output_ident,
            )));
    }

    let lang_output_ident = syn::Ident::new("LanguageOutput", Span::call_site());
    if !generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_output_ident))
    {
        generics
            .params
            .push(syn::GenericParam::Type(syn::TypeParam::from(
                lang_output_ident,
            )));
    }

    if include_language {
        let language_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == language_ident))
        {
            generics
                .params
                .push(syn::GenericParam::Type(syn::TypeParam::from(
                    language_ident,
                )));
        }
    }

    generics
}

/// Trait for global attrs that may provide a fallback format string.
pub(crate) trait HasGlobalFormat {
    fn global_format(&self) -> Option<String>;
}

impl HasGlobalFormat for crate::ChumskyGlobalAttrs {
    fn global_format(&self) -> Option<String> {
        self.format.clone()
    }
}

impl HasGlobalFormat for crate::PrettyGlobalAttrs {
    fn global_format(&self) -> Option<String> {
        None
    }
}

/// Gets the format string for a statement from a layout that uses `ChumskyStatementAttrs`.
pub(crate) fn format_for_statement<L>(
    ir_input: &kirin_derive_toolkit::ir::Input<L>,
    stmt: &kirin_derive_toolkit::ir::Statement<L>,
) -> Option<String>
where
    L: Layout<ExtraStatementAttrs = ChumskyStatementAttrs>,
    L::ExtraGlobalAttrs: HasGlobalFormat,
{
    stmt.extra_attrs
        .format
        .clone()
        .or(stmt.attrs.format.clone())
        .or(ir_input.extra_attrs.global_format())
}

/// Gets the set of field indices that are in the format string.
pub(crate) fn get_fields_in_format(
    ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
) -> HashSet<usize> {
    let Some(format_str) = format_for_statement(ir_input, stmt) else {
        return stmt.collect_fields().iter().map(|f| f.index).collect();
    };

    match Format::parse(&format_str, None) {
        Ok(format) => fields_in_format(&format, stmt),
        Err(_) => stmt.collect_fields().iter().map(|f| f.index).collect(),
    }
}

/// Collects all Value field types that contain type parameters from all statements.
pub(crate) fn collect_all_value_types_needing_bounds(
    ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    collect_value_types_needing_bounds(ir_input, &ir_input.generics)
}

/// Collects all wrapper types from structs and enum variants.
pub(crate) fn collect_wrapper_types(
    ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
) -> Vec<syn::Type> {
    match &ir_input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => data
            .0
            .wraps
            .as_ref()
            .map(|w| vec![w.ty.clone()])
            .unwrap_or_default(),
        kirin_derive_toolkit::ir::Data::Enum(data) => data
            .iter_variants()
            .filter_map(|variant| {
                if let VariantRef::Wrapper { wrapper, .. } = variant {
                    Some(wrapper.ty.clone())
                } else {
                    None
                }
            })
            .collect(),
    }
}

/// Filters collected fields to only include those needed in the AST.
pub(crate) fn filter_ast_fields<'a>(
    collected: &'a [FieldInfo<ChumskyLayout>],
    fields_in_format: &HashSet<usize>,
) -> Vec<&'a FieldInfo<ChumskyLayout>> {
    collected
        .iter()
        .filter(|f| fields_in_format.contains(&f.index) || !f.has_default())
        .collect()
}

/// Generates match arms for an enum, handling both wrapper and regular variants.
pub(crate) fn generate_enum_match<L: Layout, F, G>(
    type_name: &syn::Ident,
    data: &kirin_derive_toolkit::ir::DataEnum<L>,
    wrapper_handler: F,
    regular_handler: G,
    marker_handler: Option<TokenStream>,
) -> TokenStream
where
    F: Fn(
        &syn::Ident,
        &kirin_derive_toolkit::ir::fields::Wrapper,
        &kirin_derive_toolkit::ir::Statement<L>,
    ) -> TokenStream,
    G: Fn(&syn::Ident, &kirin_derive_toolkit::ir::Statement<L>) -> TokenStream,
{
    let arms: Vec<TokenStream> = data
        .iter_variants()
        .map(|variant| match variant {
            VariantRef::Wrapper {
                name,
                wrapper,
                stmt,
            } => {
                let body = wrapper_handler(name, wrapper, stmt);
                quote! { #type_name::#name(inner) => { #body } }
            }
            VariantRef::Regular { name, stmt } => regular_handler(name, stmt),
        })
        .collect();

    if let Some(marker) = marker_handler {
        quote! {
            match self {
                #(#arms)*
                #marker
            }
        }
    } else {
        quote! {
            match self {
                #(#arms)*
            }
        }
    }
}
