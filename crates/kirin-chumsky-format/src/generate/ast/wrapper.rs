use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::generate::collect_wrapper_types;

use super::GenerateAST;

impl GenerateAST {
    /// Generates the ASTSelf wrapper type for standalone use.
    ///
    /// This wrapper sets LanguageOutput = Self, creating a self-referential type
    /// that can be used with HasParser.
    pub(super) fn generate_ast_self_wrapper(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ir_input.name.span());
        let crate_path = &self.config.crate_path;

        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        // Build the inner AST type reference
        let inner_ast_type = if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, TypeOutput, #ast_self_name<'tokens, 'src, TypeOutput>> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* TypeOutput, #ast_self_name<'tokens, 'src, #(#type_params,)* TypeOutput>> }
        };

        // Build generics for ASTSelf definition: <'tokens, 'src, [original type params with bounds], TypeOutput>
        // Note: For the struct definition, use 'src: 'tokens bound syntax
        let ast_self_def_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src: 'tokens, TypeOutput> }
        } else {
            // Get original type parameters with their bounds
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
            quote! { <'tokens, 'src: 'tokens, #(#type_param_bounds,)* TypeOutput> }
        };

        // Build generics for impl block: no bounds on lifetimes here, just list them
        let ast_self_impl_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            // Get original type parameters with their bounds for impl
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
            quote! { <'tokens, 'src, #(#type_param_bounds,)* TypeOutput> }
        };

        // Build type reference for impl Self type: <'tokens, 'src, [params], TypeOutput>
        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            quote! { <'tokens, 'src, #(#type_params,)* TypeOutput> }
        };

        // PhantomData for unused params
        let phantom = if type_params.is_empty() {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), TypeOutput)> }
        } else {
            quote! { ::core::marker::PhantomData<fn() -> (&'tokens (), &'src (), #(#type_params,)* TypeOutput)> }
        };

        // Collect value types that need HasParser bounds
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let has_parser_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasParser<'tokens, 'src> + 'tokens })
            .collect();

        // Collect wrapper types that need HasDialectParser bounds
        let wrapper_types = collect_wrapper_types(ir_input);
        let has_dialect_parser_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();

        // The ASTSelf type needs TypeOutput: Clone + PartialEq + 'src: 'tokens
        let all_bounds: Vec<_> = has_parser_bounds
            .into_iter()
            .chain(has_dialect_parser_bounds)
            .collect();
        let where_clause = if all_bounds.is_empty() {
            quote! { where TypeOutput: Clone + PartialEq, 'src: 'tokens }
        } else {
            quote! { where TypeOutput: Clone + PartialEq, 'src: 'tokens, #(#all_bounds),* }
        };

        // Check if we need manual trait impls:
        // - If there are wrapper variants (GAT projection bounds)
        // - If there are original type parameters (to avoid incorrect bounds on phantom data)
        let has_wrapper_variants = !wrapper_types.is_empty();
        let has_original_type_params = !type_params.is_empty();
        let needs_manual_impls = has_wrapper_variants || has_original_type_params;

        if needs_manual_impls {
            // For types with wrapper variants or original type params,
            // we need manual trait impls to avoid incorrect bounds.
            //
            // For wrapper enums, the inner AST type has wrapper variants that use GAT projections.
            // For types with original params (like T: TypeLattice), #[derive] would add T: Clone
            // even though T is only in PhantomData.
            //
            // Note: Debug impl uses a placeholder for the inner type to avoid cyclic dependency:
            // - ASTSelf: Debug would require inner AST: Debug
            // - inner AST: Debug (with LanguageOutput = ASTSelf) would require ASTSelf: Debug
            // This creates an infinite cycle. Using a placeholder breaks the cycle.
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
                        // Use placeholder to avoid cyclic Debug dependency
                        // (inner AST needs LanguageOutput: Debug, which would be Self)
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
