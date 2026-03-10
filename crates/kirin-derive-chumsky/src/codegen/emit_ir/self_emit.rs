use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::{BoundsBuilder, collect_all_value_types_needing_bounds};

use super::GenerateEmitIR;

impl GenerateEmitIR {
    pub(super) fn generate_ast_self_emit_impl(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        _ast_name: &syn::Ident,
        ast_self_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &self.config.ir_type;
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        let impl_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput, Language> }
        } else {
            let type_param_bounds: Vec<_> = ir_input
                .generics
                .type_params()
                .map(|p| {
                    let ident = &p.ident;
                    let bounds = &p.bounds;
                    if bounds.is_empty() {
                        quote! { #ident }
                    } else {
                        quote! { #ident: #bounds }
                    }
                })
                .collect();
            quote! { <'t, #(#type_param_bounds,)* TypeOutput, Language> }
        };

        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput> }
        } else {
            quote! { <'t, #(#type_params,)* TypeOutput> }
        };

        let bounds = BoundsBuilder::new(crate_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.emit_ir_bounds(&value_types);

        let wrapper_types = crate::codegen::collect_wrapper_types(ir_input);
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    #ty: #crate_path::HasDialectEmitIR<'t, Language, #ast_self_name #ast_self_ty_generics>
                }
            })
            .collect();

        let ir_type_is_param = self.is_ir_type_a_type_param(ir_type, &ir_input.generics);
        let dialect_type_bound = if ir_type_is_param {
            quote! { Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let placeholder_bound = if self.needs_placeholder_bound(ir_input) {
            quote! { <Language as #ir_path::Dialect>::Type: #ir_path::Placeholder, }
        } else {
            quote! {}
        };
        let base_bounds = quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
            #dialect_type_bound
            #placeholder_bound
            TypeOutput: Clone + PartialEq,
            #ir_type: #crate_path::HasParser<'t> + 't,
            <#ir_type as #crate_path::HasParser<'t>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>,
        };

        let mut all_bounds = vec![base_bounds];
        if !value_type_bounds.is_empty() {
            let bounds_tokens = value_type_bounds.iter().map(|b| quote! { #b, });
            all_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !wrapper_emit_bounds.is_empty() {
            let emit_tokens = wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            all_bounds.push(quote! { #(#emit_tokens)* });
        }

        let where_clause = quote! { where #(#all_bounds)* };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_self_name #ast_self_ty_generics
            #where_clause
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> ::core::result::Result<Self::Output, #crate_path::EmitError> {
                    let dialect_variant = self.0.emit_with(
                        ctx,
                        &|stmt, ctx| {
                            <#ast_self_name #ast_self_ty_generics as #crate_path::EmitIR<Language>>::emit(stmt, ctx)
                        },
                    )?;
                    Ok(ctx.stage.statement().definition(dialect_variant).new())
                }
            }
        }
    }
}
