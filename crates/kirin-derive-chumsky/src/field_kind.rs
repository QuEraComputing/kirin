//! Field category extensions and code generation helpers for chumsky derive.

use std::collections::HashSet;

use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::ir::fields::{FieldCategory, FieldInfo};
use kirin_derive_toolkit::misc::{is_type, is_type_in_generic};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement, FormatOption};

/// Extension trait for [`FieldCategory`] with chumsky-specific AST generation helpers.
pub trait FieldCategoryExt {
    /// Returns the AST-dual name ("ssa_value", "result_value", "block", etc.).
    fn ast_kind_name(&self) -> &'static str;
    /// Returns the AST struct name for SSA-like categories ("SSAValue", "ResultValue"), or None.
    fn ssa_type_name(&self) -> Option<&'static str>;
}

impl FieldCategoryExt for FieldCategory {
    fn ast_kind_name(&self) -> &'static str {
        match self {
            FieldCategory::Argument => "ssa_value",
            FieldCategory::Result => "result_value",
            FieldCategory::Block => "block",
            FieldCategory::Successor => "successor",
            FieldCategory::Region => "region",
            FieldCategory::Symbol => "symbol",
            FieldCategory::Value => "value",
            FieldCategory::DiGraph | FieldCategory::UnGraph => {
                todo!("DiGraph/UnGraph fields not yet supported by parser")
            }
        }
    }

    fn ssa_type_name(&self) -> Option<&'static str> {
        match self {
            FieldCategory::Argument => Some("SSAValue"),
            FieldCategory::Result => Some("ResultValue"),
            _ => None,
        }
    }
}

/// Generates the AST type for a field.
pub fn ast_type<L: Layout>(
    field: &FieldInfo<L>,
    crate_path: &syn::Path,
    _ast_name: &syn::Ident,
    ir_type: &syn::Path,
    _type_params: &[TokenStream],
) -> TokenStream {
    let type_output = quote! { <#ir_type as #crate_path::HasParser<'t>>::Output };
    match field.category() {
        FieldCategory::Argument => {
            quote! { #crate_path::SSAValue<'t, #type_output> }
        }
        FieldCategory::Result => {
            quote! { #crate_path::ResultValue<'t, #type_output> }
        }
        FieldCategory::Block => {
            quote! { #crate_path::Spanned<#crate_path::Block<'t, #type_output, LanguageOutput>> }
        }
        FieldCategory::Successor => {
            quote! { #crate_path::BlockLabel<'t> }
        }
        FieldCategory::Region => {
            quote! { #crate_path::Region<'t, #type_output, LanguageOutput> }
        }
        FieldCategory::Symbol => {
            quote! { #crate_path::SymbolName<'t> }
        }
        FieldCategory::Value => {
            let ty = field
                .value_type()
                .cloned()
                .unwrap_or_else(|| syn::parse_quote!(()));
            quote! { <#ty as #crate_path::HasParser<'t>>::Output }
        }
        FieldCategory::DiGraph | FieldCategory::UnGraph => {
            todo!("DiGraph/UnGraph fields not yet supported by parser")
        }
    }
}

/// Generates the parser expression for a field.
pub fn parser_expr<L: Layout>(
    field: &FieldInfo<L>,
    crate_path: &syn::Path,
    opt: &FormatOption,
    _ast_name: &syn::Ident,
    ir_type: &syn::Path,
    _type_params: &[TokenStream],
) -> TokenStream {
    match field.category() {
        FieldCategory::Argument => match opt {
            FormatOption::Name => quote! { #crate_path::nameof_ssa() },
            FormatOption::Type => {
                quote! { #crate_path::typeof_ssa::<_, #ir_type>() }
            }
            FormatOption::Default => {
                quote! { #crate_path::ssa_value::<_, #ir_type>() }
            }
        },
        FieldCategory::Result => match opt {
            FormatOption::Name => quote! { #crate_path::nameof_ssa() },
            FormatOption::Type => {
                quote! { #crate_path::typeof_ssa::<_, #ir_type>() }
            }
            FormatOption::Default => {
                quote! { #crate_path::result_value::<_, #ir_type>() }
            }
        },
        FieldCategory::Block => {
            quote! {
                #crate_path::block::<_, #ir_type, _>(language.clone())
            }
        }
        FieldCategory::Successor => {
            quote! { #crate_path::block_label() }
        }
        FieldCategory::Region => {
            quote! {
                #crate_path::region::<_, #ir_type, _>(language.clone())
            }
        }
        FieldCategory::Symbol => {
            quote! { #crate_path::symbol() }
        }
        FieldCategory::Value => {
            let ty = field
                .value_type()
                .cloned()
                .unwrap_or_else(|| syn::parse_quote!(()));
            quote! { <#ty as #crate_path::HasParser<'t>>::parser() }
        }
        FieldCategory::DiGraph | FieldCategory::UnGraph => {
            todo!("DiGraph/UnGraph fields not yet supported by parser")
        }
    }
}

/// Generates pretty print expression for a field.
pub fn print_expr<L: Layout>(
    field: &FieldInfo<L>,
    prettyless_path: &syn::Path,
    field_ref: &TokenStream,
    opt: &FormatOption,
) -> TokenStream {
    match field.category() {
        FieldCategory::Argument | FieldCategory::Result => match opt {
            FormatOption::Name => quote! {
                #prettyless_path::PrettyPrint::pretty_print_name(#field_ref, doc)
            },
            FormatOption::Type => quote! {
                #prettyless_path::PrettyPrint::pretty_print_type(#field_ref, doc)
            },
            FormatOption::Default => quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            },
        },
        FieldCategory::Block => quote! {
            doc.print_block(#field_ref)
        },
        FieldCategory::Successor => quote! {
            #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
        },
        FieldCategory::Region => quote! {
            doc.print_region(#field_ref)
        },
        FieldCategory::Symbol => quote! {
            #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
        },
        FieldCategory::Value => {
            quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            }
        }
        FieldCategory::DiGraph | FieldCategory::UnGraph => {
            todo!("DiGraph/UnGraph fields not yet supported by printer")
        }
    }
}

/// Generates constructor code when only the :name format option is provided.
pub fn construct_from_name_only(
    field: &FieldInfo<impl Layout>,
    crate_path: &syn::Path,
    name_var: &syn::Ident,
) -> Option<TokenStream> {
    let type_name = syn::Ident::new(
        field.category().ssa_type_name()?,
        proc_macro2::Span::call_site(),
    );
    Some(quote! {
        #crate_path::#type_name {
            name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
            ty: None,
        }
    })
}

/// Generates constructor code when both :name and :type format options are provided.
pub fn construct_from_name_and_type(
    field: &FieldInfo<impl Layout>,
    crate_path: &syn::Path,
    name_var: &syn::Ident,
    type_var: &syn::Ident,
) -> Option<TokenStream> {
    let type_name = syn::Ident::new(
        field.category().ssa_type_name()?,
        proc_macro2::Span::call_site(),
    );
    Some(quote! {
        #crate_path::#type_name {
            name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
            ty: Some(#type_var.ty.clone()),
        }
    })
}

/// Collects Value field types that contain type parameters.
pub fn collect_value_types_needing_bounds(
    input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    generics: &syn::Generics,
) -> Vec<syn::Type> {
    let type_param_names: Vec<String> = generics
        .params
        .iter()
        .filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    if type_param_names.is_empty() {
        return Vec::new();
    }

    let mut types = Vec::new();
    let mut seen = HashSet::new();

    let statements: Vec<&kirin_derive_toolkit::ir::Statement<ChumskyLayout>> = match &input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => vec![&data.0],
        kirin_derive_toolkit::ir::Data::Enum(data) => data.variants.iter().collect(),
    };

    for stmt in statements {
        let fields = stmt.collect_fields();
        for field in &fields {
            if field.category() != FieldCategory::Value {
                continue;
            }
            if let Some(ty) = field.value_type() {
                if field.has_default() {
                    continue;
                }
                for param_name in &type_param_names {
                    if is_type(ty, param_name) || is_type_in_generic(ty, param_name) {
                        let key = quote!(#ty).to_string();
                        if seen.insert(key) {
                            types.push(ty.clone());
                        }
                        break;
                    }
                }
            }
        }
    }

    types
}

/// Returns the set of field indices that are mentioned in the format string.
pub fn fields_in_format<L: Layout>(
    format: &Format<'_>,
    stmt: &kirin_derive_toolkit::ir::Statement<L>,
) -> HashSet<usize> {
    let map_by_ident = stmt.field_name_to_index();
    let mut indices = HashSet::new();

    for elem in format.elements() {
        if let FormatElement::Field(name, _) = elem {
            // Try to parse as index first, then look up by name
            let index = name
                .parse::<usize>()
                .ok()
                .or_else(|| map_by_ident.get(*name).copied());
            if let Some(idx) = index {
                indices.insert(idx);
            }
        }
    }

    indices
}
