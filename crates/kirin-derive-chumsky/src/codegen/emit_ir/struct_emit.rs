use std::collections::HashSet;

use kirin_derive_toolkit::ir::fields::FieldInfo;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{filter_ast_fields, get_fields_in_format};

use super::GenerateEmitIR;

impl GenerateEmitIR {
    pub(super) fn generate_struct_emit(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        original_ty_generics: &syn::TypeGenerics<'_>,
        variant_name: Option<&syn::Ident>,
    ) -> TokenStream {
        let collected = stmt.collect_fields();
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let ast_fields = filter_ast_fields(&collected, &fields_in_fmt);

        let (pattern, emit_calls, constructor) = self.build_emit_components(
            ir_input,
            stmt,
            original_name,
            variant_name,
            &collected,
            &ast_fields,
            &fields_in_fmt,
            true,
        );

        quote! {
            let #pattern = self;
            #emit_calls
            let dialect_variant: #original_name #original_ty_generics = #constructor;
            ctx.stage.statement().definition(dialect_variant).new()
        }
    }

    pub(super) fn build_emit_components(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_toolkit::ir::Statement<ChumskyLayout>,
        original_name: &syn::Ident,
        variant_name: Option<&syn::Ident>,
        collected: &[FieldInfo<ChumskyLayout>],
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        fields_in_fmt: &HashSet<usize>,
        is_struct: bool,
    ) -> (TokenStream, TokenStream, TokenStream) {
        let is_tuple = stmt.is_tuple_style();

        if is_tuple {
            let mut sorted_ast_fields: Vec<_> = ast_fields.to_vec();
            sorted_ast_fields.sort_by_key(|f| f.index);

            let field_vars: Vec<_> = sorted_ast_fields
                .iter()
                .map(|f| syn::Ident::new(&format!("f{}", f.index), proc_macro2::Span::call_site()))
                .collect();

            let pattern = if is_struct {
                quote! { Self(#(#field_vars),*) }
            } else {
                quote! { #(#field_vars),* }
            };

            let emit_calls = self.generate_field_emit_calls(
                &sorted_ast_fields,
                &field_vars,
                &ir_input.generics,
                true,
            );

            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                collected,
                &sorted_ast_fields,
                &field_vars,
                fields_in_fmt,
                true,
            );

            (pattern, emit_calls, constructor)
        } else {
            let field_vars: Vec<_> = ast_fields
                .iter()
                .map(|f| {
                    let ident = f.ident.as_ref().unwrap();
                    syn::Ident::new(&format!("f_{}", ident), proc_macro2::Span::call_site())
                })
                .collect();

            let pat: Vec<_> = ast_fields
                .iter()
                .zip(&field_vars)
                .map(|(f, b)| {
                    let orig = f.ident.as_ref().unwrap();
                    quote! { #orig: #b }
                })
                .collect();

            let pattern = if is_struct {
                quote! { Self { #(#pat,)* .. } }
            } else {
                quote! { #(#pat),* }
            };

            let emit_calls =
                self.generate_field_emit_calls(ast_fields, &field_vars, &ir_input.generics, false);

            let constructor = self.generate_dialect_constructor_with_defaults(
                original_name,
                variant_name,
                collected,
                ast_fields,
                &field_vars,
                fields_in_fmt,
                false,
            );

            (pattern, emit_calls, constructor)
        }
    }
}
