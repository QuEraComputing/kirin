use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::generate::{BoundsBuilder, collect_all_value_types_needing_bounds};

use super::GenerateEmitIR;

impl GenerateEmitIR {
    /// Generates an EmitIR impl for the ASTSelf wrapper that delegates to the inner type.
    pub(super) fn generate_ast_self_emit_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_self_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let ir_path = &self.config.ir_path;
        let ir_type = &self.config.ir_type;
        let (_, original_ty_generics, _) = ir_input.generics.split_for_impl();

        // Extract original type parameters
        let type_params: Vec<TokenStream> = ir_input
            .generics
            .type_params()
            .map(|p| {
                let ident = &p.ident;
                quote! { #ident }
            })
            .collect();

        // Build generics for the ASTSelf impl: <'tokens, 'src, [original type params], TypeOutput, Language>
        let impl_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput, Language> }
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
            quote! { <'tokens, 'src, #(#type_param_bounds,)* TypeOutput, Language> }
        };

        // Build type generics for ASTSelf: <'tokens, 'src, [params], TypeOutput>
        let ast_self_ty_generics = if type_params.is_empty() {
            quote! { <'tokens, 'src, TypeOutput> }
        } else {
            quote! { <'tokens, 'src, #(#type_params,)* TypeOutput> }
        };

        // Note: We do NOT use inner_ast_type in bounds to avoid recursive trait resolution.
        // The bound `inner_ast_type: EmitIR<Language>` would cause overflow.
        let _inner_ast_type = if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, TypeOutput, #ast_self_name<'tokens, 'src, TypeOutput>> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* TypeOutput, #ast_self_name<'tokens, 'src, #(#type_params,)* TypeOutput>> }
        };

        // Use BoundsBuilder to generate EmitIR bounds
        let bounds = BoundsBuilder::new(crate_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.emit_ir_bounds(&value_types);

        // Collect wrapper types for their bounds
        let wrapper_types = crate::generate::collect_wrapper_types(ir_input);
        let wrapper_from_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { Language: ::core::convert::From<#ty> })
            .collect();
        let wrapper_dialect_parser_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| syn::parse_quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();
        // For wrapper enums, we need the inner ASTs to implement EmitIR.
        // The LanguageOutput for these is Self (the ASTSelf type), creating a coinductive requirement.
        // We use the ASTSelf type directly to allow Rust's trait solver to find the coinductive solution.
        let wrapper_emit_bounds: Vec<syn::WherePredicate> = wrapper_types
            .iter()
            .map(|ty| {
                syn::parse_quote! {
                    <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, #ast_self_name #ast_self_ty_generics>:
                        #crate_path::EmitIR<Language, Output = #ir_path::Statement>
                }
            })
            .collect();

        // Note: We do NOT add a bound requiring `inner_ast_type: EmitIR<Language>` because
        // that would create a recursive bound (ASTSelf contains AST which contains ASTSelf...).
        // Instead, we rely on the wrapper_emit_bounds which use coinduction.
        //
        // If ir_type is a type parameter, add explicit Language::Type bound
        // (same reasoning as in generate_emit_impl).
        let ir_type_is_param = self.is_ir_type_a_type_param(ir_type, &ir_input.generics);
        let dialect_type_bound = if ir_type_is_param {
            quote! { Language: #ir_path::Dialect<Type = #ir_type>, }
        } else {
            quote! {}
        };
        let base_bounds = quote! {
            Language: #ir_path::Dialect + From<#original_name #original_ty_generics>,
            #dialect_type_bound
            TypeOutput: Clone + PartialEq,
            'src: 'tokens,
            #ir_type: #crate_path::HasParser<'tokens, 'src> + 'tokens,
            <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output:
                #crate_path::EmitIR<Language, Output = <Language as #ir_path::Dialect>::Type>,
        };

        let mut all_bounds = vec![base_bounds];
        if !value_type_bounds.is_empty() {
            let bounds_tokens = value_type_bounds.iter().map(|b| quote! { #b, });
            all_bounds.push(quote! { #(#bounds_tokens)* });
        }
        if !wrapper_from_bounds.is_empty() {
            let from_tokens = wrapper_from_bounds.iter().map(|b| quote! { #b, });
            let emit_tokens = wrapper_emit_bounds.iter().map(|b| quote! { #b, });
            let dialect_parser_tokens =
                wrapper_dialect_parser_bounds.iter().map(|b| quote! { #b, });
            all_bounds
                .push(quote! { #(#from_tokens)* #(#emit_tokens)* #(#dialect_parser_tokens)* });
        }

        let where_clause = quote! { where #(#all_bounds)* };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::EmitIR<Language> for #ast_self_name #ast_self_ty_generics
            #where_clause
            {
                type Output = #ir_path::Statement;

                fn emit(&self, ctx: &mut #crate_path::EmitContext<'_, Language>) -> Self::Output {
                    // Delegate to the inner AST type - use fully qualified syntax
                    #crate_path::EmitIR::emit(&self.0, ctx)
                }
            }
        }
    }
}
