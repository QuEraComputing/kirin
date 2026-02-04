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
    /// With GAT, `HasParser::Output` is `<Self as HasDialectParser>::Output<Self>`,
    /// using Self (the dialect type) as the Language parameter to the GAT.
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
        let type_lattice = &ir_input.attrs.type_lattice;

        // Build impl generics that include both the lifetimes and the original type parameters
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses and add TypeLattice: HasParser bound
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Use BoundsBuilder to generate bounds
        let bounds = BoundsBuilder::new(crate_path, &self.config.ir_path);
        let type_lattice_bound = bounds.type_lattice_has_parser_bound(type_lattice);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.has_parser_bounds(&value_types);
        // Wrapper types need HasDialectParser bounds (no Language parameter with GAT)
        let wrapper_types = collect_wrapper_types(ir_input);
        let wrapper_type_bounds = bounds.has_dialect_parser_bounds(&wrapper_types);

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates.push(type_lattice_bound);
                wc.predicates.extend(value_type_bounds);
                wc.predicates.extend(wrapper_type_bounds);
                quote! { #wc }
            }
            None => {
                let all_bounds = std::iter::once(type_lattice_bound)
                    .chain(value_type_bounds)
                    .chain(wrapper_type_bounds)
                    .collect::<Vec<_>>();
                quote! { where #(#all_bounds),* }
            }
        };

        // The AST type for this dialect uses GAT: Output<Self>
        let dialect_type = quote! { #original_name #ty_generics };
        let ast_type = self.build_ast_type_reference(ir_input, ast_name, &dialect_type);

        quote! {
            impl #impl_generics #crate_path::HasParser<'tokens, 'src> for #original_name #ty_generics
            #where_clause
            {
                type Output = #ast_type;

                fn parser<I>() -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                {
                    use #crate_path::chumsky::prelude::*;
                    #crate_path::chumsky::recursive::recursive(|language| {
                        // For standalone use, LanguageOutput = Self::Output<Self>
                        <#original_name #ty_generics as #crate_path::HasDialectParser<
                            'tokens,
                            'src,
                        >>::recursive_parser::<I, #dialect_type, Self::Output>(language)
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
    /// With GAT, the trait has no `Language` type parameter. Instead:
    /// - `type Output<L>` is a GAT parameterized by the language type
    /// - `recursive_parser<I, L>` takes `L: Dialect` as a method type parameter
    ///
    /// This allows dialects to be composed without recursive trait bounds.
    pub(super) fn generate_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        // For wrapper structs, forward to the wrapped type's HasDialectParser impl
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self.generate_wrapper_struct_dialect_parser_impl(ir_input, wrapper, crate_path);
            }
        }

        let original_name = &ir_input.name;
        let type_lattice = &ir_input.attrs.type_lattice;
        let ir_path = &self.config.ir_path;

        // Build impl generics with just lifetimes and original type parameters (no Language)
        let impl_generics = self.build_original_type_impl_generics(ir_input);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Use BoundsBuilder to generate bounds
        let bounds = BoundsBuilder::new(crate_path, ir_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.has_parser_bounds(&value_types);
        let type_lattice_bound = bounds.type_lattice_has_parser_bound(type_lattice);
        // Wrapper types need HasDialectParser bounds (no Language parameter with GAT)
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
            wc.predicates.push(type_lattice_bound);
            wc.predicates.extend(value_type_bounds);
            wc.predicates.extend(wrapper_type_bounds);
            wc
        };

        // Generate parser body based on struct/enum
        let parser_body = self.generate_dialect_parser_body(ir_input, ast_name, crate_path);

        // The AST type for this dialect uses GAT with __Language parameter
        // Use __Language to avoid conflicts with user-defined type parameters
        let ast_type = self.build_ast_type_reference(ir_input, ast_name, &quote! { __Language });

        quote! {
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src>
                for #original_name #ty_generics
            #final_where
            {
                // GAT: Output is parameterized by __Language (top-level language dialect type)
                // Use __Language to avoid conflicts with user-defined type parameters
                type Output<__Language> = #ast_type where __Language: #ir_path::Dialect + 'tokens;
                // TypeAST is the output of parsing the type lattice via HasParser
                type TypeAST = <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output;

                #[inline]
                fn recursive_parser<I, __Language, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, __LanguageOutput>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output<__Language>>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    __Language: #ir_path::Dialect + 'tokens,
                    __LanguageOutput: Clone + 'tokens,
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

    /// Builds the fully-qualified AST type reference with a specific Language type.
    ///
    /// AST types have generics: `<'tokens, 'src, [original type params], Language>`
    /// This returns: `ASTName<'tokens, 'src, T, L, ..., LanguageType>`
    ///
    /// Common usages:
    /// - For `HasParser::Output`: pass the dialect type (e.g., `DialectName<T>`)
    /// - For `HasDialectParser::Output`: pass `quote! { Language }`
    pub(super) fn build_ast_type_reference(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        language_type: &TokenStream,
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

        // AST generics are <'tokens, 'src, [original type params], Language>
        if type_params.is_empty() {
            quote! { #ast_name<'tokens, 'src, #language_type> }
        } else {
            quote! { #ast_name<'tokens, 'src, #(#type_params,)* #language_type> }
        }
    }

    /// Generates the `HasDialectParser` impl for wrapper structs.
    ///
    /// With GAT, we forward to the wrapped type's impl using the GAT syntax:
    /// - `Output<L> = <Wrapped as HasDialectParser>::Output<L>`
    /// - `TypeAST = <Wrapped as HasDialectParser>::TypeAST`
    /// - `recursive_parser<I, L>(language) = Wrapped::recursive_parser::<I, L>(language)`
    fn generate_wrapper_struct_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        wrapper: &kirin_derive_core::ir::fields::Wrapper,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let wrapped_ty = &wrapper.ty;
        let ir_path = &self.config.ir_path;

        // Build impl generics with just lifetimes (no Language parameter with GAT)
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
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src>
                for #original_name #ty_generics
            #final_where
            {
                // GAT: Forward to wrapped type's Output GAT
                // Use __Language to avoid conflicts with user-defined type parameters
                type Output<__Language> = <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<__Language>
                    where __Language: #ir_path::Dialect + 'tokens;
                type TypeAST = <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::TypeAST;

                #[inline]
                fn recursive_parser<I, __Language, __LanguageOutput>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, __LanguageOutput>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output<__Language>>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    __Language: #ir_path::Dialect + 'tokens,
                    __LanguageOutput: Clone + 'tokens,
                {
                    <#wrapped_ty as #crate_path::HasDialectParser<'tokens, 'src>>::recursive_parser::<I, __Language, __LanguageOutput>(language)
                }
            }
        }
    }
}
