//! Field category extensions and code generation helpers for chumsky derive.

use std::collections::HashSet;

use kirin_derive_toolkit::ir::Layout;
use kirin_derive_toolkit::ir::fields::{FieldCategory, FieldInfo};
use kirin_derive_toolkit::misc::{is_type, is_type_in_generic};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::format::{BodyProjection, Format, FormatElement, FormatOption};

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
            FieldCategory::DiGraph => "digraph",
            FieldCategory::UnGraph => "ungraph",
            FieldCategory::Signature => "signature",
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
        FieldCategory::DiGraph => {
            quote! { #crate_path::DiGraph<'t, #type_output, LanguageOutput> }
        }
        FieldCategory::UnGraph => {
            quote! { #crate_path::UnGraph<'t, #type_output, LanguageOutput> }
        }
        FieldCategory::Signature => {
            // Signature<T> directly. The where clause ensures T: HasParser<'t, Output = T>.
            quote! { #crate_path::ir::Signature<#ir_type> }
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
            FormatOption::Body(_) | FormatOption::Signature(_) => {
                unreachable!("body/signature projection options are not valid on Argument fields")
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
            FormatOption::Body(_) | FormatOption::Signature(_) => {
                unreachable!("body/signature projection options are not valid on Result fields")
            }
        },
        FieldCategory::Block => match opt {
            FormatOption::Default => {
                quote! { #crate_path::block::<_, #ir_type, _>(language.clone()) }
            }
            FormatOption::Body(BodyProjection::Args) => {
                quote! { #crate_path::block_argument_list_bare::<_, #ir_type>() }
            }
            FormatOption::Body(BodyProjection::Body) => {
                quote! { #crate_path::block_body_statements(language.clone()) }
            }
            _ => unreachable!("validation prevents other projections on Block fields"),
        },
        FieldCategory::Successor => {
            quote! { #crate_path::block_label() }
        }
        FieldCategory::Region => match opt {
            FormatOption::Default => {
                quote! { #crate_path::region::<_, #ir_type, _>(language.clone()) }
            }
            FormatOption::Body(BodyProjection::Body) => {
                quote! { #crate_path::region_body::<_, #ir_type, _>(language.clone()) }
            }
            _ => unreachable!("validation prevents other projections on Region fields"),
        },
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
        FieldCategory::DiGraph => match opt {
            FormatOption::Default => {
                quote! { #crate_path::digraph::<_, #ir_type, _>(language.clone()) }
            }
            FormatOption::Body(BodyProjection::Ports) => {
                quote! { #crate_path::port_list::<_, #ir_type>() }
            }
            FormatOption::Body(BodyProjection::Captures) => {
                quote! { #crate_path::capture_list::<_, #ir_type>() }
            }
            FormatOption::Body(BodyProjection::Body) => {
                quote! { #crate_path::digraph_body_statements(language.clone()) }
            }
            _ => unreachable!("validation prevents other projections on DiGraph fields"),
        },
        FieldCategory::UnGraph => match opt {
            FormatOption::Default => {
                quote! { #crate_path::ungraph::<_, #ir_type, _>(language.clone()) }
            }
            FormatOption::Body(BodyProjection::Ports) => {
                quote! { #crate_path::port_list::<_, #ir_type>() }
            }
            FormatOption::Body(BodyProjection::Captures) => {
                quote! { #crate_path::capture_list::<_, #ir_type>() }
            }
            FormatOption::Body(BodyProjection::Body) => {
                quote! { #crate_path::ungraph_body_statements(language.clone()) }
            }
            _ => unreachable!("validation prevents other projections on UnGraph fields"),
        },
        FieldCategory::Signature => match opt {
            FormatOption::Default => {
                // Whole signature: inline the parser to avoid Output = T constraint.
                // Parses "(T, T, ...) -> T" into Signature<T>.
                quote! {
                    <#ir_type as #crate_path::HasParser<'t>>::parser()
                        .separated_by(#crate_path::chumsky::prelude::just(#crate_path::Token::Comma))
                        .collect::<::std::vec::Vec<_>>()
                        .delimited_by(
                            #crate_path::chumsky::prelude::just(#crate_path::Token::LParen),
                            #crate_path::chumsky::prelude::just(#crate_path::Token::RParen),
                        )
                        .then_ignore(#crate_path::chumsky::prelude::just(#crate_path::Token::Arrow))
                        .then(<#ir_type as #crate_path::HasParser<'t>>::parser())
                        .map(|(params, ret)| #crate_path::ir::Signature::new(params, ret, ()))
                }
            }
            FormatOption::Signature(crate::format::SignatureProjection::Inputs) => {
                // Type list: T::parser().separated_by(comma).collect::<Vec<_>>()
                quote! {
                    <#ir_type as #crate_path::HasParser<'t>>::parser()
                        .separated_by(#crate_path::chumsky::prelude::just(#crate_path::Token::Comma))
                        .collect::<::std::vec::Vec<_>>()
                }
            }
            FormatOption::Signature(crate::format::SignatureProjection::Return) => {
                // Single type: T::parser()
                quote! { <#ir_type as #crate_path::HasParser<'t>>::parser() }
            }
            _ => unreachable!("validation prevents other options on Signature fields"),
        },
    }
}

/// Generates pretty print expression for a field.
pub fn print_expr<L: Layout>(
    field: &FieldInfo<L>,
    prettyless_path: &syn::Path,
    field_ref: &TokenStream,
    opt: &FormatOption,
    ir_path: Option<&syn::Path>,
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
            // Projection options are only used with pseudo-fields (function/body),
            // never with real Argument/Result fields.
            FormatOption::Body(_) | FormatOption::Signature(_) => {
                unreachable!(
                    "body/signature projection options are not valid on Argument/Result fields"
                )
            }
        },
        FieldCategory::Block => match opt {
            FormatOption::Default => quote! { doc.print_block(#field_ref) },
            FormatOption::Body(BodyProjection::Args) => quote! {
                doc.print_block_args_only(#field_ref)
            },
            FormatOption::Body(BodyProjection::Body) => quote! {
                doc.print_block_body_only(#field_ref)
            },
            FormatOption::Body(_) => {
                unreachable!("Ports/Captures/Yields projections are not valid on Block fields")
            }
            FormatOption::Name | FormatOption::Type | FormatOption::Signature(_) => {
                unreachable!("Name/Type/Signature projections are not valid on Block fields")
            }
        },
        FieldCategory::Successor => quote! {
            #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
        },
        FieldCategory::Region => match opt {
            FormatOption::Default => quote! { doc.print_region(#field_ref) },
            FormatOption::Body(BodyProjection::Body) => quote! {
                doc.print_region_body_only(#field_ref)
            },
            FormatOption::Body(_) => {
                unreachable!(
                    "Ports/Captures/Yields/Args projections are not valid on Region fields"
                )
            }
            FormatOption::Name | FormatOption::Type | FormatOption::Signature(_) => {
                unreachable!("Name/Type/Signature projections are not valid on Region fields")
            }
        },
        FieldCategory::Symbol => quote! {
            #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
        },
        FieldCategory::Value => {
            quote! {
                #prettyless_path::PrettyPrint::pretty_print(#field_ref, doc)
            }
        }
        FieldCategory::DiGraph => match opt {
            FormatOption::Default => quote! { doc.print_digraph(#field_ref) },
            FormatOption::Body(proj) => {
                // unwrap: validate_ir_path_for_body_projections guarantees ir_path is Some
                // when DiGraph fields use body projections.
                let ir = ir_path.unwrap();
                match proj {
                    BodyProjection::Ports => quote! {
                        {
                            use #ir::GetInfo as _;
                            let __info = #field_ref.expect_info(doc.stage());
                            doc.print_ports_only(__info.ports(), __info.edge_count())
                        }
                    },
                    BodyProjection::Captures => quote! {
                        {
                            use #ir::GetInfo as _;
                            let __info = #field_ref.expect_info(doc.stage());
                            doc.print_captures_only(__info.ports(), __info.edge_count())
                        }
                    },
                    BodyProjection::Body => quote! {
                        doc.print_digraph_body_only(#field_ref)
                    },
                    BodyProjection::Args => {
                        unreachable!("BodyProjection::Args is not valid on DiGraph fields")
                    }
                }
            }
            FormatOption::Name | FormatOption::Type | FormatOption::Signature(_) => {
                unreachable!("Name/Type/Signature projections are not valid on DiGraph fields")
            }
        },
        FieldCategory::UnGraph => match opt {
            FormatOption::Default => quote! { doc.print_ungraph(#field_ref) },
            FormatOption::Body(proj) => {
                // unwrap: validate_ir_path_for_body_projections guarantees ir_path is Some
                // when UnGraph fields use body projections.
                let ir = ir_path.unwrap();
                match proj {
                    BodyProjection::Ports => quote! {
                        {
                            use #ir::GetInfo as _;
                            let __info = #field_ref.expect_info(doc.stage());
                            doc.print_ports_only(__info.ports(), __info.edge_count())
                        }
                    },
                    BodyProjection::Captures => quote! {
                        {
                            use #ir::GetInfo as _;
                            let __info = #field_ref.expect_info(doc.stage());
                            doc.print_captures_only(__info.ports(), __info.edge_count())
                        }
                    },
                    BodyProjection::Body => quote! {
                        doc.print_ungraph_body_only(#field_ref)
                    },
                    BodyProjection::Args => {
                        unreachable!("BodyProjection::Args is not valid on UnGraph fields")
                    }
                }
            }
            FormatOption::Name | FormatOption::Type | FormatOption::Signature(_) => {
                unreachable!("Name/Type/Signature projections are not valid on UnGraph fields")
            }
        },
        FieldCategory::Signature => match opt {
            FormatOption::Default => {
                // Whole signature: print (T, T) -> T
                // T: Display is guaranteed by CompileTimeValue.
                quote! {
                    doc.text(::std::format!("{}", #field_ref))
                }
            }
            FormatOption::Signature(crate::format::SignatureProjection::Inputs) => {
                // Comma-separated params
                quote! {
                    doc.list(#field_ref.params().iter(), ", ", |p| doc.text(::std::format!("{}", p)))
                }
            }
            FormatOption::Signature(crate::format::SignatureProjection::Return) => {
                // Single return type
                quote! { doc.text(::std::format!("{}", #field_ref.ret())) }
            }
            _ => unreachable!("validation prevents other options on Signature fields"),
        },
    }
}

/// Generates constructor code when only the :name format option is provided.
pub fn construct_from_name_only(
    field: &FieldInfo<impl Layout>,
    crate_path: &syn::Path,
    name_var: &syn::Ident,
    result_index: Option<usize>,
) -> Option<TokenStream> {
    let type_name = syn::Ident::new(
        field.category().ssa_type_name()?,
        proc_macro2::Span::call_site(),
    );
    let extra_fields = if field.category() == FieldCategory::Result {
        let idx = result_index.unwrap_or(0);
        quote! { result_index: #idx, }
    } else {
        quote! {}
    };
    Some(quote! {
        #crate_path::#type_name {
            name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
            ty: None,
            #extra_fields
        }
    })
}

/// Generates constructor code when both :name and :type format options are provided.
pub fn construct_from_name_and_type(
    field: &FieldInfo<impl Layout>,
    crate_path: &syn::Path,
    name_var: &syn::Ident,
    type_var: &syn::Ident,
    result_index: Option<usize>,
) -> Option<TokenStream> {
    let type_name = syn::Ident::new(
        field.category().ssa_type_name()?,
        proc_macro2::Span::call_site(),
    );
    let extra_fields = if field.category() == FieldCategory::Result {
        let idx = result_index.unwrap_or(0);
        quote! { result_index: #idx, }
    } else {
        quote! {}
    };
    Some(quote! {
        #crate_path::#type_name {
            name: #crate_path::Spanned { value: #name_var.name, span: #name_var.span },
            ty: Some(#type_var.ty.clone()),
            #extra_fields
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
