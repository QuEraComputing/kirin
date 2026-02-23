use crate::builder::statement::StatementInfo;
use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::ir::BuilderOptions;
use kirin_derive_core::ir::fields::{Collection, FieldCategory};
use kirin_derive_core::misc::{is_type, to_snake_case};
use kirin_derive_core::prelude::*;
use quote::{format_ident, quote};
use syn::spanned::Spanned;

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

pub(crate) fn build_result_module_name(input: &InputMeta) -> syn::Ident {
    format_ident!(
        "{}_build_result",
        to_snake_case(input.name.to_string()),
        span = input.name.span()
    )
}

fn build_result_path(input: &InputMeta, info: &StatementInfo) -> proc_macro2::TokenStream {
    let mod_name = build_result_module_name(input);
    let name = &info.name;
    quote! { #mod_name::#name }
}

fn build_fn_inputs(info: &StatementInfo) -> Vec<proc_macro2::TokenStream> {
    let mut inputs = Vec::new();
    for field in info.fields.iter() {
        match field.category() {
            FieldCategory::Result => continue,
            FieldCategory::Value => {
                if field.has_default() {
                    continue;
                }
                let ty = field.value_type().expect("Value field must have type");
                let name = field.name_ident(ty.span());
                let sig = if field.has_into() {
                    quote! { #name: impl Into<#ty> }
                } else {
                    quote! { #name: #ty }
                };
                inputs.push(sig);
            }
            FieldCategory::Argument
            | FieldCategory::Block
            | FieldCategory::Successor
            | FieldCategory::Region
            | FieldCategory::Symbol => {
                let ty = field_type_for_category(&field.collection, field.category());
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
        match field.category() {
            FieldCategory::Result => continue,
            FieldCategory::Value => {
                let ty = field.value_type().expect("Value field must have type");
                let name = field.name_ident(ty.span());
                if let Some(default_value) = field.default_value() {
                    let expr = default_value.to_expr();
                    assigns.push(quote! { let #name: #ty = #expr; });
                } else if field.has_into() {
                    assigns.push(quote! { let #name: #ty = #name.into(); });
                } else if is_type(ty, "PhantomData") {
                    assigns.push(
                        syn::Error::new_spanned(
                            ty,
                            "use `#[kirin(default)]` to initialize PhantomData fields",
                        )
                        .to_compile_error(),
                    );
                } else {
                    assigns.push(quote! {});
                }
            }
            FieldCategory::Argument
            | FieldCategory::Block
            | FieldCategory::Successor
            | FieldCategory::Region
            | FieldCategory::Symbol => {
                let ty = field_type_for_category(&field.collection, field.category());
                let name = field.name_ident(ty.span());
                assigns.push(quote! { let #name: #ty = #name.into(); });
            }
        }
    }
    assigns
}

fn field_type_for_category(collection: &Collection, category: FieldCategory) -> syn::Type {
    let base = match category {
        FieldCategory::Argument => "SSAValue",
        FieldCategory::Result => "ResultValue",
        FieldCategory::Block => "Block",
        FieldCategory::Successor => "Successor",
        FieldCategory::Region => "Region",
        FieldCategory::Symbol => "Symbol",
        FieldCategory::Value => {
            unreachable!("field_type_for_category does not support Value")
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
        .filter(|f| f.category() == FieldCategory::Result)
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

fn statement_id_name(info: &StatementInfo) -> syn::Ident {
    let name = info.name.to_string().to_lowercase();
    format_ident!("{}_statement_id", name, span = info.name.span())
}

fn build_fn_body(
    info: &StatementInfo,
    input: &InputMeta,
    result_name_map: &std::collections::HashMap<usize, syn::Ident>,
) -> proc_macro2::TokenStream {
    let statement_id = statement_id_name(info);
    let let_inputs = build_fn_let_inputs(info);
    let let_results = let_name_eq_result_value(info, result_name_map);

    // Use ConstructorBuilder to generate the constructor expression
    let is_tuple = info.fields.iter().all(|f| f.ident.is_none());
    let constructor = if input.is_enum {
        ConstructorBuilder::new_variant(&input.name, &info.name, is_tuple).build_with_self(
            &info.fields,
            |field| {
                if let Some(name) = result_name_map.get(&field.index) {
                    quote! { #name }
                } else {
                    let name = field.name_ident(info.name.span());
                    quote! { #name }
                }
            },
        )
    } else {
        ConstructorBuilder::new_struct(&input.name, is_tuple).build_with_self(
            &info.fields,
            |field| {
                if let Some(name) = result_name_map.get(&field.index) {
                    quote! { #name }
                } else {
                    let name = field.name_ident(info.name.span());
                    quote! { #name }
                }
            },
        )
    };

    let build_result_path = build_result_path(input, info);
    let result_names = result_names(info);

    quote! {{
        let #statement_id = stage.statement_arena().next_id();
        #(#let_inputs)*
        #let_results

        stage
            .statement()
            .definition(#constructor)
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
        if field.category() != FieldCategory::Result {
            continue;
        }
        let collection = &field.collection;
        let ssa_ty = field.ssa_type().expect("Result field must have ssa_type");

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
            let #name: ResultValue = stage
                .ssa()
                .kind(SSAKind::Result(#statement_id, #index))
                .ty(Lang::Type::from(#ssa_ty))
                .new()
                .into();
        });
    }
    quote! { #(#results)* }
}

pub(crate) fn build_fn_for_statement(
    info: &StatementInfo,
    input: &InputMeta,
    crate_path: &syn::Path,
    is_enum: bool,
) -> darling::Result<proc_macro2::TokenStream> {
    if info.is_wrapper {
        return Ok(proc_macro2::TokenStream::new());
    }

    let inputs = build_fn_inputs(info);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.name;
    let ir_type = &input.ir_type;
    let build_fn_name = &info.build_fn_name;
    let build_result_path = build_result_path(input, info);
    let result_names = result_names(info);
    let result_name_map = info
        .fields
        .iter()
        .filter(|f| f.category() == FieldCategory::Result)
        .zip(result_names.iter().cloned())
        .map(|(field, name)| (field.index, name))
        .collect::<std::collections::HashMap<_, _>>();
    let body = build_fn_body(info, input, &result_name_map);
    let self_ty = quote! { #name #ty_generics };

    let fn_tokens = quote! {
        pub fn #build_fn_name<Lang>(stage: &mut #crate_path::StageInfo<Lang>, #(#inputs),*) -> #build_result_path
        where
            Lang: #crate_path::Dialect + From<#self_ty>,
            Lang::Type: From<#ir_type>
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
    input: &InputMeta,
    info: &StatementInfo,
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    build_fn_for_statement(info, input, crate_path, false).unwrap_or_else(|err| err.write_errors())
}

pub(crate) fn enum_build_fn<F>(
    input: &InputMeta,
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
        #[automatically_derived]
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
        .filter(|f| f.category() == FieldCategory::Result)
        .collect::<Vec<_>>();

    for (field, name) in results.into_iter().zip(names.into_iter()) {
        if matches!(field.collection, Collection::Vec) {
            fields.push(
                syn::Error::new_spanned(
                    field.name_ident(info.name.span()),
                    "ResultValue field cannot be a Vec, consider implementing the builder manually",
                )
                .to_compile_error(),
            );
            continue;
        } else if matches!(field.collection, Collection::Option) {
            fields.push(
                    syn::Error::new_spanned(
                        field.name_ident(info.name.span()),
                        "ResultValue field cannot be an Option, consider implementing the builder manually",
                    )
                    .to_compile_error(),
                );
            continue;
        }

        fields.push(quote! {
            pub #name: ResultValue,
        });
    }

    Ok(quote! {
        #[automatically_derived]
        #[doc(hidden)]
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
    input: &InputMeta,
    info: &StatementInfo,
    statement: &ir::Statement<StandardLayout>,
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    let mod_name = build_result_module_name(input);
    let build_result_impl =
        build_result_impl(info, statement).unwrap_or_else(|err| err.write_errors());

    quote! {
        #[automatically_derived]
        #[doc(hidden)]
        pub mod #mod_name {
            use #crate_path::{Statement, ResultValue};
            #build_result_impl
        }
    }
}

pub(crate) fn build_result_module_enum<F>(
    input: &InputMeta,
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
        #[doc(hidden)]
        pub mod #mod_name {
            use #crate_path::{Statement, ResultValue};
            #(#impls)*
        }
    })
}

pub(crate) fn from_impl(input: &InputMeta, info: &StatementInfo) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.name;
    // Get wrapper type from StatementInfo
    let wrapper_ty = info
        .wrapper_type
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));

    // For wrapper variants, we need to generate let statements for each field
    // The wrapper field gets `value`, other fields get defaults
    let let_name_eq_input: Vec<_> = info
        .fields
        .iter()
        .map(|field| {
            let field_name = field.name_ident(info.name.span());
            match field.category() {
                FieldCategory::Value => {
                    if let Some(default_value) = field.default_value() {
                        let expr = default_value.to_expr();
                        quote! { let #field_name = #expr }
                    } else {
                        quote! { let #field_name = ::core::default::Default::default() }
                    }
                }
                _ => quote! { let #field_name = Default::default() },
            }
        })
        .collect();

    // Build the constructor using ConstructorBuilder
    let is_tuple = info.fields.iter().all(|f| f.ident.is_none());

    // For wrapper impl, we need to handle differently - just construct with `value`
    // Since wrapper fields are not in `fields` anymore, we just use the wrapper_type directly
    let init_head = if input.is_enum {
        let variant_name = &info.name;
        quote! { Self::#variant_name }
    } else {
        quote! { Self }
    };

    // For wrappers, typically there's a single field that takes `value`
    // Check if fields is empty (pure wrapper) or has additional fields
    if info.fields.is_empty() {
        // Pure wrapper - just wrap the value
        let initialization = if is_tuple || info.fields.is_empty() {
            quote! { (value) }
        } else {
            // This shouldn't happen for pure wrappers
            quote! { { value } }
        };

        quote! {
            #[automatically_derived]
            impl #impl_generics From<#wrapper_ty> for #name #ty_generics #where_clause {
                fn from(value: #wrapper_ty) -> Self {
                    #init_head #initialization
                }
            }
        }
    } else {
        // Wrapper with additional fields - this is a more complex case
        // Use ConstructorBuilder for the constructor
        let constructor = if input.is_enum {
            ConstructorBuilder::new_variant(&input.name, &info.name, is_tuple).build_with_self(
                &info.fields,
                |field| {
                    let field_name = field.name_ident(info.name.span());
                    quote! { #field_name }
                },
            )
        } else {
            ConstructorBuilder::new_struct(&input.name, is_tuple).build_with_self(
                &info.fields,
                |field| {
                    let field_name = field.name_ident(info.name.span());
                    quote! { #field_name }
                },
            )
        };

        quote! {
        #[automatically_derived]
        impl #impl_generics From<#wrapper_ty> for #name #ty_generics #where_clause {
            fn from(value: #wrapper_ty) -> Self {
                #(#let_name_eq_input;)*
                    #constructor
                }
            }
        }
    }
}
