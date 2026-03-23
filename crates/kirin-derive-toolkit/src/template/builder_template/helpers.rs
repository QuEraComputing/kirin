use crate::codegen::ConstructorBuilder;
use crate::context::InputMeta;
use crate::ir::fields::{Collection, FieldCategory, FieldIndex, FieldInfo};
use crate::ir::{self, BuilderOptions, StandardLayout};
use crate::misc::{is_type, to_snake_case};
use quote::{format_ident, quote};
use syn::spanned::Spanned;

#[derive(Clone, Debug)]
pub(super) struct StatementInfo {
    pub(super) name: syn::Ident,
    pub(super) fields: Vec<FieldInfo<StandardLayout>>,
    pub(super) build_fn_name: syn::Ident,
    pub(super) is_wrapper: bool,
    pub(super) wrapper_type: Option<syn::Type>,
    pub(super) wrapper_field: Option<FieldIndex>,
}

pub(super) fn build_fn_name(
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

fn build_result_module_name(input: &InputMeta) -> syn::Ident {
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

fn build_fn_inputs(info: &StatementInfo, ir_type: &syn::Path) -> Vec<proc_macro2::TokenStream> {
    let result_names = result_names(info);
    let result_fields: Vec<_> = info
        .fields
        .iter()
        .filter(|f| f.category() == FieldCategory::Result)
        .collect();
    let result_name_iter = result_fields.iter().zip(result_names.iter());

    let mut inputs = Vec::new();
    // Add count/flag parameters for Vec/Option result fields
    for (field, name) in result_name_iter {
        match field.collection {
            Collection::Vec => {
                let count_param = format_ident!("{}_count", name);
                inputs.push(quote! { #count_param: usize });
            }
            Collection::Option => {
                let has_param = format_ident!("has_{}", name);
                inputs.push(quote! { #has_param: bool });
            }
            Collection::Single => {}
        }
    }

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
            FieldCategory::Signature => {
                let ty = field_type_for_signature(&field.collection, ir_type);
                let name = field.name_ident(proc_macro2::Span::call_site());
                inputs.push(quote! { #name: impl Into<#ty> });
            }
            FieldCategory::Argument
            | FieldCategory::Block
            | FieldCategory::Successor
            | FieldCategory::Region
            | FieldCategory::DiGraph
            | FieldCategory::UnGraph
            | FieldCategory::Symbol => {
                let ty = field_type_for_category(&field.collection, field.category());
                let name = field.name_ident(ty.span());
                inputs.push(quote! { #name: impl Into<#ty> });
            }
        }
    }
    inputs
}

fn build_fn_let_inputs(info: &StatementInfo, ir_type: &syn::Path) -> Vec<proc_macro2::TokenStream> {
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
            FieldCategory::Signature => {
                let ty = field_type_for_signature(&field.collection, ir_type);
                let name = field.name_ident(proc_macro2::Span::call_site());
                assigns.push(quote! { let #name: #ty = #name.into(); });
            }
            FieldCategory::Argument
            | FieldCategory::Block
            | FieldCategory::Successor
            | FieldCategory::Region
            | FieldCategory::DiGraph
            | FieldCategory::UnGraph
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
        FieldCategory::DiGraph => "DiGraph",
        FieldCategory::UnGraph => "UnGraph",
        FieldCategory::Symbol => "Symbol",
        FieldCategory::Signature => {
            unreachable!("use field_type_for_signature for Signature fields")
        }
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

/// Returns the type for a Signature field, parameterized by the ir_type.
fn field_type_for_signature(collection: &Collection, ir_type: &syn::Path) -> syn::Type {
    match collection {
        Collection::Single => syn::parse_quote!(Signature<#ir_type>),
        Collection::Vec => syn::parse_quote!(Vec<Signature<#ir_type>>),
        Collection::Option => syn::parse_quote!(Option<Signature<#ir_type>>),
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
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    let statement_id = statement_id_name(info);
    let let_inputs = build_fn_let_inputs(info, &input.ir_type);
    let let_results = let_name_eq_result_value(info, result_name_map, crate_path);

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

/// Returns `true` if any Result field in `info` uses a dynamic collection
/// (`Vec` or `Option`), requiring a runtime result-index counter.
fn has_dynamic_result_index(info: &StatementInfo) -> bool {
    info.fields.iter().any(|f| {
        f.category() == FieldCategory::Result && !matches!(f.collection, Collection::Single)
    })
}

fn let_name_eq_result_value(
    info: &StatementInfo,
    result_name_map: &std::collections::HashMap<usize, syn::Ident>,
    crate_path: &syn::Path,
) -> proc_macro2::TokenStream {
    let mut results = Vec::new();
    let statement_id = statement_id_name(info);
    let dynamic = has_dynamic_result_index(info);

    // When any Vec/Option result field is present, we track result_index at runtime.
    // Otherwise we use compile-time literal indices.
    let mut static_index = 0usize;
    if dynamic {
        results.push(quote! {
            let mut __result_index: usize = 0;
        });
    }

    for field in info.fields.iter() {
        if field.category() != FieldCategory::Result {
            continue;
        }
        let collection = &field.collection;
        let ssa_ty = field.ssa_type().expect("Result field must have ssa_type");
        let name = result_name_map
            .get(&field.index)
            .cloned()
            .unwrap_or_else(|| format_ident!("result", span = info.name.span()));

        match collection {
            Collection::Single => {
                let index_expr = if dynamic {
                    quote! { __result_index }
                } else {
                    let idx = static_index;
                    quote! { #idx }
                };
                results.push(quote! {
                    let #name: ResultValue = stage
                        .ssa()
                        .kind(#crate_path::BuilderSSAKind::Result(#statement_id, #index_expr))
                        .ty(Lang::Type::from(#ssa_ty))
                        .new()
                        .into();
                });
                static_index += 1;
                if dynamic {
                    results.push(quote! { __result_index += 1; });
                }
            }
            Collection::Vec => {
                let count_param = format_ident!("{}_count", name);
                results.push(quote! {
                    let #name: Vec<ResultValue> = (0..#count_param).map(|__i| {
                        stage
                            .ssa()
                            .kind(#crate_path::BuilderSSAKind::Result(#statement_id, __result_index + __i))
                            .ty(Lang::Type::from(#ssa_ty))
                            .new()
                            .into()
                    }).collect();
                    __result_index += #count_param;
                });
            }
            Collection::Option => {
                let has_param = format_ident!("has_{}", name);
                results.push(quote! {
                    let #name: Option<ResultValue> = if #has_param {
                        let __val = stage
                            .ssa()
                            .kind(#crate_path::BuilderSSAKind::Result(#statement_id, __result_index))
                            .ty(Lang::Type::from(#ssa_ty))
                            .new()
                            .into();
                        __result_index += 1;
                        Some(__val)
                    } else {
                        None
                    };
                });
            }
        }
    }
    quote! { #(#results)* }
}

pub(super) fn build_fn_for_statement(
    info: &StatementInfo,
    input: &InputMeta,
    crate_path: &syn::Path,
    is_enum: bool,
) -> darling::Result<proc_macro2::TokenStream> {
    if info.is_wrapper {
        return Ok(proc_macro2::TokenStream::new());
    }

    let inputs = build_fn_inputs(info, &input.ir_type);
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
    let body = build_fn_body(info, input, &result_name_map, crate_path);
    let self_ty = quote! { #name #ty_generics };

    let needs_placeholder_bound = info.fields.iter().any(|f| f.is_auto_placeholder());

    let placeholder_bound = if needs_placeholder_bound {
        quote! { , #ir_type: #crate_path::Placeholder }
    } else {
        quote! {}
    };

    let fn_tokens = quote! {
        pub fn #build_fn_name<Lang>(stage: &mut impl #crate_path::AsBuildStage<Lang>, #(#inputs),*) -> #build_result_path
        where
            Lang: #crate_path::Dialect + From<#self_ty>,
            Lang::Type: From<#ir_type>
            #placeholder_bound
        {
            let stage = stage.as_build_stage();
            #body
        }
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

pub(super) fn enum_build_fn<F>(
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

pub(super) fn build_result_impl(
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
        match field.collection {
            Collection::Single => {
                fields.push(quote! {
                    pub #name: ResultValue,
                });
            }
            Collection::Vec => {
                fields.push(quote! {
                    pub #name: Vec<ResultValue>,
                });
            }
            Collection::Option => {
                fields.push(quote! {
                    pub #name: Option<ResultValue>,
                });
            }
        }
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

pub(super) fn build_result_module(
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

pub(super) fn build_result_module_enum<F>(
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

pub(super) fn from_impl(input: &InputMeta, info: &StatementInfo) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.name;
    let wrapper_ty = info
        .wrapper_type
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(()));

    // Determine if the wrapper field itself is a tuple (positional) field.
    let wrapper_is_tuple = info
        .wrapper_field
        .as_ref()
        .map(|f| f.ident.is_none())
        .unwrap_or(true);

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

    let init_head = if input.is_enum {
        let variant_name = &info.name;
        quote! { Self::#variant_name }
    } else {
        quote! { Self }
    };

    if info.fields.is_empty() {
        // P1-11: Use wrapper_is_tuple to determine tuple vs named constructor.
        let initialization = if wrapper_is_tuple {
            quote! { (value) }
        } else {
            let wrapper_name = info.wrapper_field.as_ref().unwrap().name();
            quote! { { #wrapper_name: value } }
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
        // P1-10: Build constructor that includes BOTH the wrapper field and extra fields.
        let wrapper_field_ref = info
            .wrapper_field
            .as_ref()
            .expect("wrapper field should be present when is_wrapper is true");

        let constructor = if wrapper_is_tuple {
            // Tuple variant/struct: position matters. Build a full positional list
            // where the wrapper index gets `value` and extras get their defaulted names.
            let wrapper_idx = wrapper_field_ref.index;
            let total_fields = info.fields.len() + 1;
            let mut values = Vec::with_capacity(total_fields);
            let mut extra_iter = info.fields.iter();
            for i in 0..total_fields {
                if i == wrapper_idx {
                    values.push(quote! { value });
                } else {
                    let field = extra_iter
                        .next()
                        .expect("field count mismatch in tuple From impl");
                    let field_name = field.name_ident(info.name.span());
                    values.push(quote! { #field_name });
                }
            }
            if input.is_enum {
                let variant_name = &info.name;
                quote! { Self::#variant_name(#(#values),*) }
            } else {
                quote! { Self(#(#values),*) }
            }
        } else {
            // Named variant/struct: emit all field assignments including wrapper.
            let wrapper_name = wrapper_field_ref.name();
            let extra_assigns: Vec<_> = info
                .fields
                .iter()
                .map(|field| {
                    let field_name = field.name_ident(info.name.span());
                    quote! { #field_name: #field_name }
                })
                .collect();
            if input.is_enum {
                let variant_name = &info.name;
                quote! { Self::#variant_name { #wrapper_name: value, #(#extra_assigns),* } }
            } else {
                quote! { Self { #wrapper_name: value, #(#extra_assigns),* } }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StandardLayout;
    use crate::ir::fields::{Collection, FieldData};
    use quote::format_ident;

    fn make_result_field(
        index: usize,
        name: &str,
        collection: Collection,
    ) -> FieldInfo<StandardLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection,
            data: FieldData::Result {
                ssa_type: syn::parse_quote!(MyType::placeholder()),
                is_auto_placeholder: true,
            },
        }
    }

    fn make_argument_field(index: usize, name: &str) -> FieldInfo<StandardLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Argument {
                ssa_type: syn::parse_quote!(MyType::placeholder()),
            },
        }
    }

    fn make_statement_info(name: &str, fields: Vec<FieldInfo<StandardLayout>>) -> StatementInfo {
        StatementInfo {
            name: format_ident!("{}", name),
            fields,
            build_fn_name: format_ident!("op_{}", name.to_lowercase()),
            is_wrapper: false,
            wrapper_type: None,
            wrapper_field: None,
        }
    }

    fn make_ir_statement(name: &str) -> ir::Statement<StandardLayout> {
        let attrs = ir::StatementOptions {
            format: None,
            builder: None,
            constant: false,
            pure: false,
            speculatable: false,
            terminator: false,
            edge: false,
        };
        ir::Statement::new(format_ident!("{}", name), attrs, (), (), vec![])
    }

    fn result_name_map_from(info: &StatementInfo) -> std::collections::HashMap<usize, syn::Ident> {
        let names = result_names(info);
        info.fields
            .iter()
            .filter(|f| f.category() == FieldCategory::Result)
            .zip(names.into_iter())
            .map(|(field, name)| (field.index, name))
            .collect()
    }

    #[test]
    fn vec_result_value_let_name_codegen() {
        let info = make_statement_info(
            "Call",
            vec![
                make_argument_field(0, "callee"),
                make_result_field(1, "results", Collection::Vec),
            ],
        );
        let name_map = result_name_map_from(&info);
        let crate_path: syn::Path = syn::parse_quote!(kirin_ir);
        let tokens = let_name_eq_result_value(&info, &name_map, &crate_path);
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("vec_result_value_let_name", formatted);
    }

    #[test]
    fn option_result_value_let_name_codegen() {
        let info = make_statement_info(
            "IfOp",
            vec![
                make_argument_field(0, "condition"),
                make_result_field(1, "result", Collection::Option),
            ],
        );
        let name_map = result_name_map_from(&info);
        let crate_path: syn::Path = syn::parse_quote!(kirin_ir);
        let tokens = let_name_eq_result_value(&info, &name_map, &crate_path);
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("option_result_value_let_name", formatted);
    }

    #[test]
    fn vec_result_value_build_result_codegen() {
        let info = make_statement_info(
            "Call",
            vec![
                make_argument_field(0, "callee"),
                make_result_field(1, "results", Collection::Vec),
            ],
        );
        let statement = make_ir_statement("Call");
        let tokens = build_result_impl(&info, &statement).unwrap();
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("vec_result_value_build_result", formatted);
    }

    #[test]
    fn option_result_value_build_result_codegen() {
        let info = make_statement_info(
            "IfOp",
            vec![
                make_argument_field(0, "condition"),
                make_result_field(1, "result", Collection::Option),
            ],
        );
        let statement = make_ir_statement("IfOp");
        let tokens = build_result_impl(&info, &statement).unwrap();
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("option_result_value_build_result", formatted);
    }

    #[test]
    fn single_result_value_unchanged() {
        // Regression test: single ResultValue should continue to work exactly as before
        let info = make_statement_info(
            "Add",
            vec![
                make_argument_field(0, "lhs"),
                make_argument_field(1, "rhs"),
                make_result_field(2, "result", Collection::Single),
            ],
        );
        let name_map = result_name_map_from(&info);
        let crate_path: syn::Path = syn::parse_quote!(kirin_ir);
        let tokens = let_name_eq_result_value(&info, &name_map, &crate_path);
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("single_result_value_let_name", formatted);
    }

    #[test]
    fn vec_result_value_build_fn_inputs_codegen() {
        let info = make_statement_info(
            "Call",
            vec![
                make_argument_field(0, "callee"),
                make_result_field(1, "results", Collection::Vec),
            ],
        );
        let ir_type: syn::Path = syn::parse_quote!(MyType);
        let inputs = build_fn_inputs(&info, &ir_type);
        let tokens = quote! { #(#inputs),* };
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("vec_result_value_build_fn_inputs", formatted);
    }

    #[test]
    fn option_result_value_build_fn_inputs_codegen() {
        let info = make_statement_info(
            "IfOp",
            vec![
                make_argument_field(0, "condition"),
                make_result_field(1, "result", Collection::Option),
            ],
        );
        let ir_type: syn::Path = syn::parse_quote!(MyType);
        let inputs = build_fn_inputs(&info, &ir_type);
        let tokens = quote! { #(#inputs),* };
        let formatted = crate::test_util::rustfmt_tokens(&tokens);
        insta::assert_snapshot!("option_result_value_build_fn_inputs", formatted);
    }
}
