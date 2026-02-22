//! Trait implementation generation for HasParser and HasDialectParser.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::generics::GenericsBuilder;

use kirin_derive_core::codegen::combine_where_clauses;

use super::super::{BoundsBuilder, collect_all_value_types_needing_bounds, collect_wrapper_types};
use super::GenerateHasDialectParser;

impl GenerateHasDialectParser {
    /// Generates the `HasParser` impl for the original type.
    /// This provides the `parser()` method that sets up recursive parsing.
    ///
    /// With the new design, `HasParser::Output` is the ASTSelf wrapper type.
    pub(super) fn generate_has_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        // For wrapper structs, forward to the wrapped type's HasParser impl
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self.generate_wrapper_struct_has_parser_impl(ir_input, wrapper, crate_path);
            }
        }

        let original_name = &ir_input.name;
        let ir_type = &ir_input.attrs.ir_type;

        // Build impl generics that include both the lifetimes and the original type parameters
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses and add IR type: HasParser bound
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Use BoundsBuilder to generate bounds
        let bounds = BoundsBuilder::new(crate_path, &self.config.ir_path);
        let ir_type_bound = bounds.ir_type_has_parser_bound(ir_type);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.has_parser_bounds(&value_types);
        // Wrapper types need HasDialectParser bounds
        let wrapper_types = collect_wrapper_types(ir_input);
        let wrapper_type_bounds = bounds.has_dialect_parser_bounds(&wrapper_types);

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates.push(ir_type_bound);
                wc.predicates.extend(value_type_bounds);
                wc.predicates.extend(wrapper_type_bounds);
                quote! { #wc }
            }
            None => {
                let all_bounds = std::iter::once(ir_type_bound)
                    .chain(value_type_bounds)
                    .chain(wrapper_type_bounds)
                    .collect::<Vec<_>>();
                quote! { where #(#all_bounds),* }
            }
        };

        // The ASTSelf wrapper type for standalone use
        let ast_self_name = syn::Ident::new(&format!("{}Self", ast_name), ast_name.span());
        let ast_self_type = self.build_ast_self_type_reference(ir_input, &ast_self_name, ir_type);
        let type_output = quote! { <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParser<'tokens, 'src> for #original_name #ty_generics
            #where_clause
            {
                type Output = #ast_self_type;

                fn parser<I>() -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    use #crate_path::chumsky::prelude::*;
                    #crate_path::chumsky::recursive::recursive(|language| {
                        // For standalone use, LanguageOutput = Self::Output (the ASTSelf type)
                        <#original_name #ty_generics as #crate_path::HasDialectParser<
                            'tokens,
                            'src,
                        >>::recursive_parser::<I, #type_output, Self::Output>(language)
                            .map(|inner| #ast_self_name::new(inner))
                    }).boxed()
                }
            }
        }
    }

    /// Generates the `HasParser` impl for wrapper structs.
    ///
    /// For wrapper structs, we forward completely to the wrapped type's impl:
    /// - `Output = <Wrapped as HasParser>::Output`
    /// - `parser() = <Wrapped as HasParser>::parser()`
    fn generate_wrapper_struct_has_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;

        // Build impl generics
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // The wrapped type needs HasParser bound
        let wrapped_bound: syn::WherePredicate =
            syn::parse_quote! { #wrapped_ty: #crate_path::HasParser<'tokens, 'src> };

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates.push(wrapped_bound);
                quote! { #wc }
            }
            None => {
                quote! { where #wrapped_bound }
            }
        };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasParser<'tokens, 'src> for #original_name #ty_generics
            #where_clause
            {
                type Output = <#wrapped_ty as #crate_path::HasParser<'tokens, 'src>>::Output;

                fn parser<I>() -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    <#wrapped_ty as #crate_path::HasParser<'tokens, 'src>>::parser()
                }
            }
        }
    }

    /// Generates the `HasDialectParser` impl for the dialect type.
    ///
    /// With the new design:
    /// - `type Output<TypeOutput, LanguageOutput>` has two type parameters
    /// - `recursive_parser<I, TypeOutput, LanguageOutput>` takes both as method type parameters
    ///
    /// This allows dialects to be composed without GAT projection issues.
    pub(super) fn generate_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        // For wrapper structs, forward to the wrapped type's HasDialectParser impl
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self
                    .generate_wrapper_struct_dialect_parser_impl(ir_input, wrapper, crate_path);
            }
        }

        let original_name = &ir_input.name;
        let ir_type = &ir_input.attrs.ir_type;

        // Build impl generics with just lifetimes and original type parameters
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Use BoundsBuilder to generate bounds
        let bounds = BoundsBuilder::new(crate_path, &self.config.ir_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.has_parser_bounds(&value_types);
        let ir_type_bound = bounds.ir_type_has_parser_bound(ir_type);
        // Wrapper types need HasDialectParser bounds
        let wrapper_types = collect_wrapper_types(ir_input);
        let wrapper_type_bounds = bounds.has_dialect_parser_bounds(&wrapper_types);

        let final_where = {
            let mut wc = match combined_where {
                Some(wc) => wc,
                None => syn::WhereClause {
                    where_token: syn::token::Where::default(),
                    predicates: syn::punctuated::Punctuated::new(),
                },
            };
            wc.predicates.push(ir_type_bound);
            wc.predicates.extend(value_type_bounds);
            wc.predicates.extend(wrapper_type_bounds);
            wc
        };

        // Generate parser body based on struct/enum
        let parser_body = self.generate_dialect_parser_body(ir_input, ast_name, crate_path);

        // The AST type with TypeOutput and LanguageOutput parameters
        let ast_type = self.build_ast_type_with_type_params(ir_input, ast_name);

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src>
                for #original_name #ty_generics
            #final_where
            {
                // Output is parameterized by TypeOutput and LanguageOutput
                type Output<__TypeOutput, __LanguageOutput> = #ast_type
                where
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __LanguageOutput: Clone + PartialEq + 'tokens;

                #[inline]
                fn recursive_parser<I, __TypeOutput, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, __LanguageOutput>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output<__TypeOutput, __LanguageOutput>>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __LanguageOutput: Clone + PartialEq + 'tokens,
                {
                    use #crate_path::chumsky::prelude::*;
                    #parser_body.boxed()
                }
            }
        }
    }

    /// Builds impl generics for the original type's HasDialectParser impl.
    pub(super) fn build_original_type_impl_generics(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
    ) -> syn::Generics {
        GenericsBuilder::new(&self.config.ir_path).with_lifetimes(&ir_input.generics)
    }

    /// Builds the AST type reference with __TypeOutput and __LanguageOutput parameters.
    ///
    /// Returns: `ASTName<'tokens, 'src, [original type params], __TypeOutput, __LanguageOutput>`
    fn build_ast_type_with_type_params(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
    ) -> TokenStream {
        // Extract just the type parameters from the original generics (not lifetimes)
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        // AST generics are <'tokens, 'src, [original type params], __TypeOutput, __LanguageOutput>
        if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, __TypeOutput, __LanguageOutput> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* __TypeOutput, __LanguageOutput> }
        }
    }

    /// Builds the ASTSelf type reference for HasParser::Output.
    ///
    /// Returns: `ASTNameSelf<'tokens, 'src, [original type params], IrTypeOutput>`
    fn build_ast_self_type_reference(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_self_name: &syn::Ident,
        ir_type: &syn::Path,
    ) -> TokenStream {
        let crate_path = &self.config.crate_path;

        // Extract just the type parameters from the original generics (not lifetimes)
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        let type_output = quote! { <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output };

        // ASTSelf generics are <'tokens, 'src, [original type params], TypeOutput>
        if type_params.is_empty() {
            quote! { #ast_self_name<'tokens, 'src, #type_output> }
        } else {
            quote! { #ast_self_name<'tokens, 'src, #(#type_params,)* #type_output> }
        }
    }

    /// Builds the fully-qualified AST type reference with specific TypeOutput and LanguageOutput.
    ///
    /// This is used in parser bodies for return type annotations.
    pub(super) fn build_ast_type_reference(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        type_output: &TokenStream,
        language_output: &TokenStream,
    ) -> TokenStream {
        // Extract just the type parameters from the original generics (not lifetimes)
        let type_params: Vec<_> = ir_input
            .generics
            .params
            .iter()
            .filter_map(|p| {
                if let syn::GenericParam::Type(tp) = p {
                    let ident = &tp.ident;
                    Some(quote! { #ident })
                } else {
                    None
                }
            })
            .collect();

        // AST generics are <'tokens, 'src, [original type params], TypeOutput, LanguageOutput>
        if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, #type_output, #language_output> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* #type_output, #language_output> }
        }
    }

    /// Generates the `HasDialectParser` impl for wrapper structs.
    ///
    /// We forward to the wrapped type's impl:
    /// - `Output<TypeOutput, LanguageOutput> = <Wrapped as HasDialectParser>::Output<TypeOutput, LanguageOutput>`
    /// - `recursive_parser` forwards to wrapped type
    fn generate_wrapper_struct_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;

        // Build impl generics with just lifetimes
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();
        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Build bound: Wrapped: HasDialectParser
        let wrapped_bound: syn::WherePredicate =
            syn::parse_quote! { #wrapped_ty: #crate_path::HasDialectParser<'tokens, 'src> };

        let final_where = {
            let mut wc = match combined_where {
                Some(wc) => wc,
                None => syn::WhereClause {
                    where_token: syn::token::Where::default(),
                    predicates: syn::punctuated::Punctuated::new(),
                },
            };
            wc.predicates.push(wrapped_bound);
            wc
        };

        quote! {
            #[automatically_derived]
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src>
                for #original_name #ty_generics
            #final_where
            {
                // Forward to wrapped type's Output
                type Output<__TypeOutput, __LanguageOutput> =
                    <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<__TypeOutput, __LanguageOutput>
                where
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __LanguageOutput: Clone + PartialEq + 'tokens;

                #[inline]
                fn recursive_parser<I, __TypeOutput, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, __LanguageOutput>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output<__TypeOutput, __LanguageOutput>>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    __TypeOutput: Clone + PartialEq + 'tokens,
                    __LanguageOutput: Clone + PartialEq + 'tokens,
                {
                    <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::recursive_parser::<I, __TypeOutput, __LanguageOutput>(language)
                }
            }
        }
    }
}
