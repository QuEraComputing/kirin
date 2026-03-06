use std::collections::HashSet;

use kirin_derive_toolkit::ir::fields::FieldInfo;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{filter_ast_fields, generate_enum_match, get_fields_in_format};

use super::GenerateEmitIR;

impl GenerateEmitIR {
    pub(super) fn generate_enum_emit(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        data: &kirin_derive_toolkit::ir::DataEnum<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let marker = quote! {
            #ast_name::__Marker(_, unreachable) => match *unreachable {}
        };

        let crate_path = &self.config.crate_path;
        let crate_path_for_match = crate_path.clone();
        generate_enum_match(
            ast_name,
            data,
            move |_name, _wrapper| {
                quote! { #crate_path_for_match::EmitIR::emit(inner, ctx) }
            },
            |name, variant| {
                self.generate_variant_emit(
                    ir_input,
                    variant,
                    original_name,
                    original_ty_generics,
                    ast_name,
                    name,
                )
            },
            Some(marker),
        )
    }

    fn generate_variant_emit(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        variant: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        ast_name: &syn::Ident,
        variant_name: &syn::Ident,
    ) -> TokenStream {
        let collected = variant.collect_fields();
        let fields_in_fmt = get_fields_in_format(ir_input, variant);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);
        let is_tuple = variant.is_tuple_style();

        let (pattern, emit_calls, constructor) = self.build_emit_components(
            ir_input,
            variant,
            original_name,
            Some(variant_name),
            &collected,
            &ast_fields,
            &fields_in_fmt,
            false,
        );

        let full_pattern = if ast_fields.is_empty() {
            if is_tuple {
                quote! { #ast_name::#variant_name }
            } else {
                quote! { #ast_name::#variant_name {} }
            }
        } else if is_tuple {
            quote! { #ast_name::#variant_name(#pattern) }
        } else {
            quote! { #ast_name::#variant_name { #pattern } }
        };

        quote! {
            #full_pattern => {
                #emit_calls
                let dialect_variant: #original_name #original_ty_generics = #constructor;
                ctx.stage.statement().definition(dialect_variant).new()
            }
        }
    }

    pub(super) fn generate_dialect_constructor_with_defaults(
        &self,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
        all_fields: &[FieldInfo<ChumskyLayout>],
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        field_vars: &[syn::Ident],
        _fields_in_fmt: &HashSet<usize>,
        is_tuple: bool,
    ) -> TokenStream {
        use kirin_derive_toolkit::ir::fields::FieldCategory;

        let ast_field_vars: std::collections::HashMap<usize, &syn::Ident> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(f, v)| (f.index, v))
            .collect();

        let ordered_all_fields: Vec<_> = if is_tuple {
            let mut sorted: Vec<_> = all_fields.iter().collect();
            sorted.sort_by_key(|f| f.index);
            sorted
        } else {
            all_fields.iter().collect()
        };

        let field_values: Vec<_> = ordered_all_fields
            .iter()
            .map(|field| {
                if let Some(var) = ast_field_vars.get(&field.index) {
                    let emitted_var =
                        syn::Ident::new(&format!("{}_ir", var), proc_macro2::Span::call_site());

                    match field.category() {
                        FieldCategory::Argument
                        | FieldCategory::Result
                        | FieldCategory::Block
                        | FieldCategory::Successor
                        | FieldCategory::Region
                        | FieldCategory::Symbol => {
                            quote! { #emitted_var.into() }
                        }
                        FieldCategory::Value => {
                            quote! { #emitted_var }
                        }
                    }
                } else if let Some(default_value) = field.default_value() {
                    let default_expr = default_value.to_expr();
                    quote! { #default_expr }
                } else {
                    quote! { ::core::default::Default::default() }
                }
            })
            .collect();

        if is_tuple {
            match variant_name {
                Some(v) => quote! { #original_name::#v(#(#field_values),*) },
                None => quote! { #original_name(#(#field_values),*) },
            }
        } else {
            let field_assigns: Vec<_> = ordered_all_fields
                .iter()
                .zip(field_values.iter())
                .map(|(field, value)| {
                    let name = field.ident.as_ref().unwrap();
                    quote! { #name: #value }
                })
                .collect();

            match variant_name {
                Some(v) => quote! { #original_name::#v { #(#field_assigns),* } },
                None => quote! { #original_name { #(#field_assigns),* } },
            }
        }
    }
}
