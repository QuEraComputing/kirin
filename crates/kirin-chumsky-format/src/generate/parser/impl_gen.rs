//! Trait implementation generation for HasParser and HasDialectParser.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::generics::GenericsBuilder;

use kirin_derive_core::codegen::combine_where_clauses;

use super::super::{BoundsBuilder, collect_all_value_types_needing_bounds};
use super::GenerateHasDialectParser;

impl GenerateHasDialectParser {
    /// Generates the `HasParser` impl for the original type.
    /// This provides the `parser()` method that sets up recursive parsing.
    pub(super) fn generate_has_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
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

        let where_clause = match combined_where {
            Some(mut wc) => {
                wc.predicates.push(type_lattice_bound);
                wc.predicates.extend(value_type_bounds);
                quote! { #wc }
            }
            None => {
                let all_bounds = std::iter::once(type_lattice_bound)
                    .chain(value_type_bounds)
                    .collect::<Vec<_>>();
                quote! { where #(#all_bounds),* }
            }
        };

        // The AST type for this dialect (Language = Self)
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
                        <#original_name #ty_generics as #crate_path::HasDialectParser<
                            'tokens,
                            'src,
                            #original_name #ty_generics,
                        >>::recursive_parser(language)
                    }).boxed()
                }
            }
        }
    }

    /// Generates the `HasDialectParser` impl for the dialect type.
    ///
    /// Only the dialect type implements `HasDialectParser`. The AST type is just the Output.
    /// The impl is generic over `Language` to allow this dialect to be embedded in a larger
    /// language composition rather than always being the top-level language.
    pub(super) fn generate_dialect_parser_impl(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        ast_name: &syn::Ident,
        crate_path: &syn::Path,
    ) -> TokenStream {
        let original_name = &ir_input.name;
        let type_lattice = &ir_input.attrs.type_lattice;
        let ir_path = &self.config.ir_path;

        // Build impl generics that include lifetimes, original type parameters, and Language
        // Language is added without bounds here; the Dialect bound is in the where clause
        let impl_generics =
            GenericsBuilder::new(&self.config.ir_path).with_language_unbounded(&ir_input.generics);
        let (impl_generics, _, impl_where_clause) = impl_generics.split_for_impl();

        let (_, ty_generics, where_clause) = ir_input.generics.split_for_impl();

        // Combine where clauses
        let combined_where = combine_where_clauses(where_clause, impl_where_clause);

        // Use BoundsBuilder to generate bounds
        let bounds = BoundsBuilder::new(crate_path, ir_path);
        let value_types = collect_all_value_types_needing_bounds(ir_input);
        let value_type_bounds = bounds.has_parser_bounds(&value_types);
        let type_lattice_bound = bounds.type_lattice_has_parser_bound(type_lattice);
        let language_dialect_bound = bounds.language_dialect_bound();

        let final_where = {
            let mut wc = match combined_where {
                Some(wc) => wc,
                None => syn::WhereClause {
                    where_token: syn::token::Where::default(),
                    predicates: syn::punctuated::Punctuated::new(),
                },
            };
            wc.predicates.push(language_dialect_bound);
            wc.predicates.push(type_lattice_bound);
            wc.predicates.extend(value_type_bounds);
            wc
        };

        // Generate parser body based on struct/enum
        let parser_body = self.generate_dialect_parser_body(ir_input, ast_name, crate_path);

        // The AST type for this dialect, using generic Language parameter
        let language = quote! { Language };
        let ast_type = self.build_ast_type_reference(ir_input, ast_name, &language);

        // The Language's output type (for the recursive parser argument)
        let language_output =
            quote! { <Language as #crate_path::HasDialectParser<'tokens, 'src, Language>>::Output };

        quote! {
            impl #impl_generics #crate_path::HasDialectParser<'tokens, 'src, Language>
                for #original_name #ty_generics
            #final_where
            {
                type Output = #ast_type;
                // TypeAST is the output of parsing the type lattice via HasParser
                type TypeAST = <#type_lattice as #crate_path::HasParser<'tokens, 'src>>::Output;

                #[inline]
                fn recursive_parser<I>(
                    language: #crate_path::RecursiveParser<'tokens, 'src, I, #language_output>,
                ) -> #crate_path::BoxedParser<'tokens, 'src, I, Self::Output>
                where
                    I: #crate_path::TokenInput<'tokens, 'src>,
                    Language: #crate_path::HasDialectParser<'tokens, 'src, Language>,
                {
                    use #crate_path::chumsky::prelude::*;
                    // SAFETY: The transmute converts between two identical types:
                    // - #ast_type (the concrete AST type with explicit lifetimes)
                    // - Self::Output (defined as `type Output = #ast_type` above)
                    //
                    // This transmute is necessary because Rust's type system treats associated
                    // types as opaque during type checking. Even though `type Output = #ast_type`
                    // is defined in this impl block, Rust cannot unify the concrete type with
                    // `Self::Output` for type inference purposes. The types are identical by
                    // construction, so this transmute is safe.
                    let parser: #crate_path::BoxedParser<'tokens, 'src, I, #ast_type> = #parser_body.boxed();
                    unsafe { ::core::mem::transmute(parser) }
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
}
