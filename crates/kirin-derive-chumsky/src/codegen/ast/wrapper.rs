use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::codegen::collect_wrapper_types;

use super::GenerateAST;

impl GenerateAST {
    /// Generates the ASTSelf wrapper type for standalone use.
    pub(super) fn generate_ast_self_wrapper(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ir_input.name.span());
        let crate_path = &self.config.crate_path;

        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        let inner_ast_type = if type_params.is_empty() {
            quote! { #ast_name<'t, TypeOutput, #ast_self_name<'t, TypeOutput>> }
        } else {
            quote! { #ast_name<'t, #(#type_params,)* TypeOutput, #ast_self_name<'t, #(#type_params,)* TypeOutput>> }
        };

        let ast_self_def_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput> }
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
            quote! { <'t, #(#type_param_bounds,)* TypeOutput> }
        };

        let ast_self_impl_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput> }
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
            quote! { <'t, #(#type_param_bounds,)* TypeOutput> }
        };

        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'t, TypeOutput> }
        } else {
            quote! { <'t, #(#type_params,)* TypeOutput> }
        };

        let phantom = if type_params.is_empty() {
            quote! { ::core::marker::PhantomData<fn() -> (&'t (), TypeOutput)> }
        } else {
            quote! { ::core::marker::PhantomData<fn() -> (&'t (), #(#type_params,)* TypeOutput)> }
        };

        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'t> + 't })
            .collect();

        let wrapper_types = collect_wrapper_types(ir_input);
        let has_dialect_parser_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'t> })
            .collect();

        let all_bounds: Vec<_> = has_parser_bounds
            .into_iter()
            .chain(has_dialect_parser_bounds)
            .collect();
        let where_clause = if all_bounds.is_empty() {
            quote! { where TypeOutput: Clone + PartialEq }
        } else {
            quote! { where TypeOutput: Clone + PartialEq, #(#all_bounds),* }
        };

        let has_wrapper_variants = !wrapper_types.is_empty();
        let has_original_type_params = !type_params.is_empty();
        let needs_manual_impls = has_wrapper_variants || has_original_type_params;

        if needs_manual_impls {
            let ast_self_name_str = ast_self_name.to_string();

            quote! {
                #[doc(hidden)]
                pub struct #ast_self_name #ast_self_def_generics (
                    pub #inner_ast_type,
                    #phantom,
                ) #where_clause;

                impl #ast_self_impl_generics Clone for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        Self(self.0.clone(), ::core::marker::PhantomData)
                    }
                }

                impl #ast_self_impl_generics ::core::fmt::Debug for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        f.debug_tuple(#ast_self_name_str)
                            .field(&"..")
                            .finish()
                    }
                }

                impl #ast_self_impl_generics PartialEq for #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        self.0 == other.0
                    }
                }

                impl #ast_self_impl_generics #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    /// Creates a new ASTSelf wrapper.
                    pub fn new(inner: #inner_ast_type) -> Self {
                        Self(inner, ::core::marker::PhantomData)
                    }
                }
            }
        } else {
            quote! {
                #[derive(Clone, Debug, PartialEq)]
                #[doc(hidden)]
                pub struct #ast_self_name #ast_self_def_generics (
                    pub #inner_ast_type,
                    #phantom,
                ) #where_clause;

                impl #ast_self_impl_generics #ast_self_name #ast_self_ty_generics
                #where_clause
                {
                    /// Creates a new ASTSelf wrapper.
                    pub fn new(inner: #inner_ast_type) -> Self {
                        Self(inner, ::core::marker::PhantomData)
                    }
                }
            }
        }
    }
}
