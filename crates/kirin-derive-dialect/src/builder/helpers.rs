use crate::builder::statement::{FieldInfo, FieldKind, StatementInfo};
use kirin_derive_core::derive::InputContext;
use kirin_derive_core::ir::BuilderOptions;
use kirin_derive_core::ir::fields::Collection;
use kirin_derive_core::misc::{is_type, to_snake_case};
use kirin_derive_core::prelude::*;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

#[derive(Clone, Copy, Debug)]
enum FieldStyle {
    Named,
    Unnamed,
    Unit,
}

pub(crate) fn build_fn_name(
    is_enum: bool,
    statement: &ir::Statement<StandardLayout>,
) -> syn::Ident {
    let default_name = if is_enum {
        format_ident!(
            "op_{}",
            to_snake_case(statement.name.to_string()),
            span = statement.name.span()
        )
    } else {
        format_ident!("new", span = statement.name.span())
    };

    match &statement.attrs.builder {
        Some(BuilderOptions::Named(name)) => {
            format_ident!("{}", name, span = statement.name.span())
        }
        _ => default_name,
    }
}

pub(crate) fn build_result_module_name(input: &InputContext) -> syn::Ident {
    format_ident!(
        "{}_build_result",
        to_snake_case(input.name.to_string()),
        span = input.name.span()
    )
}

fn build_result_path(input: &InputContext, info: &StatementInfo) -> proc_macro2::TokenStream {
    let mod_name = build_result_module_name(input);
    let name = &info.name;
    quote! { #mod_name::#name }
}

fn build_fn_inputs(info: &StatementInfo) -> Vec<proc_macro2::TokenStream> {
    let mut inputs = Vec::new();
    for field in info.fields.iter() {
        match &field.kind {
            FieldKind::Result { .. } => continue,
            FieldKind::Value { ty, default, into } => {
                if default.is_some() {
                    continue;
                }
                let name = field.name_ident(ty.span());
                let sig = if *into {
                    quote! { #name: impl Into<#ty> }
                } else {
                    quote! { #name: #ty }
                };
                inputs.push(sig);
            }
            FieldKind::Wrapper { .. } => continue,
            FieldKind::Argument { collection }
            | FieldKind::Block { collection }
            | FieldKind::Successor { collection }
            | FieldKind::Region { collection } => {
                let ty = field_type_for_kind(collection, &field.kind);
                let name = field.name_ident(ty.span());
                inputs.push(quote! { #name: impl Into<#ty> });
            }
        }
    }
    inputs
}

fn build_fn_let_inputs(info: &StatementInfo) -> Vec<proc_macro2::TokenStream> {
    let mut assigns = Vec::new();
    for field in info.fields.iter() {
        match &field.kind {
            FieldKind::Result { .. } => continue,
            FieldKind::Wrapper { .. } => continue,
            FieldKind::Value { ty, default, into } => {
                let name = field.name_ident(ty.span());
                if let Some(expr) = default.as_ref() {
                    assigns.push(quote! { let #name: #ty = #expr; });
                } else if *into {
                    assigns.push(quote! { let #name: #ty = #name.into(); });
                } else if is_type(ty, "PhantomData") {
                    assigns.push(
                        syn::Error::new_spanned(
                            ty,
                            "use `#[kirin(default = std::marker::PhantomData)]` \
                            to initialize PhantomData fields",
                        )
                        .to_compile_error(),
                    );
                } else {
                    assigns.push(quote! {});
                }
            }
            FieldKind::Argument { collection }
            | FieldKind::Block { collection }
            | FieldKind::Successor { collection }
            | FieldKind::Region { collection } => {
                let ty = field_type_for_kind(collection, &field.kind);
                let name = field.name_ident(ty.span());
                assigns.push(quote! { let #name: #ty = #name.into(); });
            }
        }
    }
    assigns
}

fn field_type_for_kind(collection: &Collection, kind: &FieldKind) -> syn::Type {
    let base = match kind {
        FieldKind::Argument { .. } => "SSAValue",
        FieldKind::Result { .. } => "ResultValue",
        FieldKind::Block { .. } => "Block",
        FieldKind::Successor { .. } => "Successor",
        FieldKind::Region { .. } => "Region",
        FieldKind::Wrapper { .. } | FieldKind::Value { .. } => {
            unreachable!("field_type_for_kind only supports statement reference kinds")
        }
    };
    let base_ident: syn::Ident = syn::parse_str(base).unwrap();
    match collection {
        Collection::Single => syn::parse_quote!(#base_ident),
        Collection::Vec => syn::parse_quote!(Vec<#base_ident>),
        Collection::Option => syn::parse_quote!(Option<#base_ident>),
    }
}

fn result_names(info: &StatementInfo) -> Vec<syn::Ident> {
    let results: Vec<_> = info
        .fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Result { .. }))
        .collect();
    if results.len() == 1 {
        let field = results[0];
        return vec![
            field
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("result", span = info.name.span())),
        ];
    }

    results
        .iter()
        .enumerate()
        .map(|(index, field)| {
            field
                .ident
                .clone()
                .unwrap_or_else(|| format_ident!("result_{}", index, span = info.name.span()))
        })
        .collect()
}

fn initialization_tokens(
    info: &StatementInfo,
    field_style: FieldStyle,
    result_name_map: &std::collections::HashMap<usize, syn::Ident>,
) -> proc_macro2::TokenStream {
    let names: Vec<_> = info
        .fields
        .iter()
        .map(|field| {
            if let Some(name) = result_name_map.get(&field.index) {
                name.clone()
            } else {
                field.name_ident(info.name.span())
            }
        })
        .collect();

    match field_style {
        FieldStyle::Named => quote! { { #(#names,)* } },
        FieldStyle::Unnamed => quote! { ( #(#names,)* ) },
        FieldStyle::Unit => quote! {},
    }
}

fn statement_id_name(info: &StatementInfo) -> syn::Ident {
    let name = info.name.to_string().to_lowercase();
    format_ident!("{}_statement_id", name, span = info.name.span())
}

fn build_fn_body(
    info: &StatementInfo,
    input: &InputContext,
    field_style: FieldStyle,
    result_name_map: &std::collections::HashMap<usize, syn::Ident>,
) -> proc_macro2::TokenStream {
    let statement_id = statement_id_name(info);
    let let_inputs = build_fn_let_inputs(info);
    let let_results = let_name_eq_result_value(info, result_name_map);
    let init_head = if input.is_enum {
        let name = &info.name;
        quote! { Self::#name }
    } else {
        quote! { Self }
    };
    let initialization = initialization_tokens(info, field_style, result_name_map);
    let build_result_path = build_result_path(input, info);
    let result_names = result_names(info);

    quote! {{
        let #statement_id = context.statement_arena().next_id();
        #(#let_inputs)*
        #let_results

        context
            .statement()
            .definition(#init_head #initialization)
            .new();

        #build_result_path {
            id: #statement_id,
            #(#result_names),*
        }
    }}
}

fn let_name_eq_result_value(
    info: &StatementInfo,
    result_name_map: &std::collections::HashMap<usize, syn::Ident>,
) -> proc_macro2::TokenStream {
    let mut results = Vec::new();
    let statement_id = statement_id_name(info);
    let mut result_index = 0usize;
    for field in info.fields.iter() {
        let FieldKind::Result { collection, ssa_ty } = &field.kind else {
            continue;
        };
        if matches!(collection, Collection::Vec) {
            results.push(
                syn::Error::new_spanned(
                    field.name_ident(info.name.span()),
                    "ResultValue field cannot be a Vec, consider implementing the builder manually",
                )
                .to_compile_error(),
            );
            continue;
        } else if matches!(collection, Collection::Option) {
            results.push(
                syn::Error::new_spanned(
                    field.name_ident(info.name.span()),
                    "ResultValue field cannot be an Option, consider implementing the builder manually",
                )
                .to_compile_error(),
            );
            continue;
        }

        let name = result_name_map
            .get(&field.index)
            .cloned()
            .unwrap_or_else(|| format_ident!("result", span = info.name.span()));
        let index = result_index;
        result_index += 1;
        results.push(quote! {
            let #name: ResultValue = context
                .ssa()
                .kind(SSAKind::Result(#statement_id, #index))
                .ty(Lang::TypeLattice::from(#ssa_ty))
                .new()
                .into();
        });
    }
    quote! { #(#results)* }
}

fn field_style(fields: &[FieldInfo]) -> FieldStyle {
    if fields.is_empty() {
        return FieldStyle::Unit;
    }
    let has_named = fields.iter().any(|f| f.ident.is_some());
    if has_named {
        FieldStyle::Named
    } else {
        FieldStyle::Unnamed
    }
}

pub(crate) fn build_fn_for_statement(
    info: &StatementInfo,
    input: &InputContext,
    crate_path: &syn::Path,
    is_enum: bool,
) -> darling::Result<proc_macro2::TokenStream> {
    if info.is_wrapper {
        return Ok(proc_macro2::TokenStream::new());
    }

    let inputs = build_fn_inputs(info);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.name;
    let type_lattice = &input.type_lattice;
    let build_fn_name = &info.build_fn_name;
    let build_result_path = build_result_path(input, info);
    let style = field_style(&info.fields);
    let result_names = result_names(info);
    let result_name_map = info
        .fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Result { .. }))
        .zip(result_names.iter().cloned())
        .map(|(field, name)| (field.index, name))
        .collect::<std::collections::HashMap<_, _>>();
    let body = build_fn_body(info, input, style, &result_name_map);
    let self_ty = quote! { #name #ty_generics };

    let fn_tokens = quote! {
        pub fn #build_fn_name<Lang>(context: &mut #crate_path::Context<Lang>, #(#inputs),*) -> #build_result_path
        where
            Lang: #crate_path::Dialect + From<#self_ty>,
            Lang::TypeLattice: From<#type_lattice>
        #body
    };

    Ok(if is_enum {
        fn_tokens
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics #name #ty_generics #where_clause {
                #fn_tokens
            }
        }
    })
}

pub(crate) fn struct_build_fn(
    input: &InputContext,
    info: &StatementInfo,
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    build_fn_for_statement(info, input, crate_path, false).unwrap_or_else(|err| err.write_errors())
}

pub(crate) fn enum_build_fn<F>(
    input: &InputContext,
    data: &ir::DataEnum<StandardLayout>,
    mut build_fn: F,
) -> darling::Result<proc_macro2::TokenStream>
where
    F: FnMut(&ir::Statement<StandardLayout>) -> darling::Result<proc_macro2::TokenStream>,
{
    let name = &input.name;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let mut functions = Vec::new();
    for statement in &data.variants {
        let tokens = build_fn(statement)?;
        if !tokens.is_empty() {
            functions.push(tokens);
        }
    }
    if functions.is_empty() {
        return Ok(proc_macro2::TokenStream::new());
    }

    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#functions)*
        }
    })
}

pub(crate) fn build_result_impl(
    info: &StatementInfo,
    statement: &ir::Statement<StandardLayout>,
) -> darling::Result<proc_macro2::TokenStream> {
    if statement.wraps.is_some() {
        return Ok(proc_macro2::TokenStream::new());
    }

    let build_result_name = &info.name;
    let mut fields = Vec::new();
    let names = result_names(info);
    let results = info
        .fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Result { .. }))
        .collect::<Vec<_>>();

    for (field, name) in results.into_iter().zip(names.into_iter()) {
        if let FieldKind::Result { collection, .. } = &field.kind {
            if matches!(collection, Collection::Vec) {
                fields.push(
                    syn::Error::new_spanned(
                        field.name_ident(info.name.span()),
                        "ResultValue field cannot be a Vec, consider implementing the builder manually",
                    )
                    .to_compile_error(),
                );
                continue;
            } else if matches!(collection, Collection::Option) {
                fields.push(
                    syn::Error::new_spanned(
                        field.name_ident(info.name.span()),
                        "ResultValue field cannot be an Option, consider implementing the builder manually",
                    )
                    .to_compile_error(),
                );
                continue;
            }
        }

        fields.push(quote! {
            pub #name: ResultValue,
        });
    }

    Ok(quote! {
        #[automatically_derived]
        pub struct #build_result_name {
            pub id: Statement,
            #(#fields)*
        }

        #[automatically_derived]
        impl From<#build_result_name> for Statement {
            fn from(value: #build_result_name) -> Self {
                value.id
            }
        }
    })
}

pub(crate) fn build_result_module(
    input: &InputContext,
    info: &StatementInfo,
    statement: &ir::Statement<StandardLayout>,
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    let mod_name = build_result_module_name(input);
    let build_result_impl =
        build_result_impl(info, statement).unwrap_or_else(|err| err.write_errors());

    quote! {
        #[automatically_derived]
        pub mod #mod_name {
            use #crate_path::{Statement, ResultValue};
            #build_result_impl
        }
    }
}

pub(crate) fn build_result_module_enum<F>(
    input: &InputContext,
    data: &ir::DataEnum<StandardLayout>,
    crate_path: &syn::Path,
    mut build_impl: F,
) -> darling::Result<proc_macro2::TokenStream>
where
    F: FnMut(&ir::Statement<StandardLayout>) -> darling::Result<proc_macro2::TokenStream>,
{
    let mod_name = build_result_module_name(input);
    let mut impls = Vec::new();
    for statement in &data.variants {
        impls.push(build_impl(statement)?);
    }
    Ok(quote! {
        #[automatically_derived]
        pub mod #mod_name {
            use #crate_path::{Statement, ResultValue};
            #(#impls)*
        }
    })
}

pub(crate) fn from_impl(input: &InputContext, info: &StatementInfo) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.name;
    let wrapper_ty = info
        .fields
        .iter()
        .find_map(|field| {
            if let FieldKind::Wrapper { ty } = &field.kind {
                Some(ty.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| syn::parse_quote!(()));

    let let_name_eq_input: Vec<_> = info
        .fields
        .iter()
        .map(|field| {
            let name = field.name_ident(info.name.span());
            match &field.kind {
                FieldKind::Wrapper { .. } => quote! { let #name = value },
                FieldKind::Value { default, .. } => {
                    if let Some(expr) = default.as_ref() {
                        quote! { let #name = #expr }
                    } else {
                        quote! { let #name = Default::default() }
                    }
                }
                _ => quote! { let #name = Default::default() },
            }
        })
        .collect();

    let init_head = if input.is_enum {
        let name = &info.name;
        quote! { Self::#name }
    } else {
        quote! { Self }
    };
    let style = field_style(&info.fields);
    let result_name_map = std::collections::HashMap::new();
    let initialization = initialization_tokens(info, style, &result_name_map);

    quote! {
        impl #impl_generics From<#wrapper_ty> for #name #ty_generics #where_clause {
            fn from(value: #wrapper_ty) -> Self {
                #(#let_name_eq_input;)*
                #init_head #initialization
            }
        }
    }
}
