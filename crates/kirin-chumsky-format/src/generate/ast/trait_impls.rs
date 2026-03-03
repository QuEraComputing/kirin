use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;
use crate::field_kind::collect_fields;

use crate::generate::{
    collect_wrapper_types, filter_ast_fields, get_fields_in_format,
};

use super::GenerateAST;

impl GenerateAST {
    /// Generates manual Clone, Debug, PartialEq implementations for structs.
    ///
    /// This is needed when the struct has original type parameters (like `T: TypeLattice`)
    /// because standard #[derive] adds bounds on ALL type params, but we only want bounds
    /// on specific types (TypeOutput, LanguageOutput, value types).
    pub(super) fn generate_manual_struct_trait_impls(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        base_bounds: &TokenStream,
        has_parser_bounds: &[TokenStream],
        is_tuple: bool,
    ) -> TokenStream {
        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();
        let crate_path = &self.config.crate_path;
        let ir_type = &self.config.ir_type;

        // Collect value types that need Debug bounds (types containing type parameters)
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let value_debug_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug })
            .collect();

        // Base where clause for Clone and PartialEq (no Debug bounds)
        let where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
        };

        // Debug where clause adds Debug bounds on the actual field types:
        // - AST fields use <ir_type as HasParser>::Output for type annotations
        // - Value fields use <ValueType as HasParser>::Output
        // - Block/Region fields use LanguageOutput for statements
        // So we need Debug on all these types.
        let debug_where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug,
                LanguageOutput: ::core::fmt::Debug,
                #(#value_debug_bounds,)*
        };

        // Get field info for pattern matching
        let collected = collect_fields(stmt);
        let fields_in_fmt = get_fields_in_format(ir_input, stmt);
        let filtered: Vec<_> = filter_ast_fields(&collected, &fields_in_fmt);

        if is_tuple {
            // Tuple struct
            let field_count = filtered.len();
            let field_indices: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                .collect();
            let clone_fields: Vec<_> = field_indices
                .iter()
                .map(|f| quote! { #f.clone() })
                .collect();
            let debug_fields: Vec<_> = field_indices
                .iter()
                .map(|f| quote! { .field(#f) })
                .collect();
            let eq_a: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("a{}", i), proc_macro2::Span::call_site()))
                .collect();
            let eq_b: Vec<_> = (0..field_count)
                .map(|i| syn::Ident::new(&format!("b{}", i), proc_macro2::Span::call_site()))
                .collect();
            let eq_comparisons: Vec<_> = eq_a
                .iter()
                .zip(eq_b.iter())
                .map(|(a, b)| quote! { #a == #b })
                .collect();
            let eq_comparison = if eq_comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#eq_comparisons)&&* }
            };

            let ast_name_str = ast_name.to_string();

            quote! {
                impl #impl_generics Clone for #ast_name #ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        let Self(#(#field_indices,)* _marker) = self;
                        Self(#(#clone_fields,)* ::core::marker::PhantomData)
                    }
                }

                impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
                #debug_where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        let Self(#(#field_indices,)* _) = self;
                        f.debug_tuple(#ast_name_str)#(#debug_fields)*.finish()
                    }
                }

                impl #impl_generics PartialEq for #ast_name #ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        let Self(#(#eq_a,)* _) = self;
                        let Self(#(#eq_b,)* _) = other;
                        #eq_comparison
                    }
                }
            }
        } else {
            // Named struct
            let field_names: Vec<_> = filtered.iter().filter_map(|f| f.ident.as_ref()).collect();
            let clone_fields: Vec<_> = field_names
                .iter()
                .map(|f| quote! { #f: self.#f.clone() })
                .collect();
            let debug_fields: Vec<_> = field_names
                .iter()
                .map(|f| {
                    let name_str = f.to_string();
                    quote! { .field(#name_str, &self.#f) }
                })
                .collect();
            let eq_comparisons: Vec<_> = field_names
                .iter()
                .map(|f| quote! { self.#f == other.#f })
                .collect();
            let eq_comparison = if eq_comparisons.is_empty() {
                quote! { true }
            } else {
                quote! { #(#eq_comparisons)&&* }
            };

            let ast_name_str = ast_name.to_string();

            quote! {
                impl #impl_generics Clone for #ast_name #ty_generics
                #where_clause
                {
                    fn clone(&self) -> Self {
                        Self {
                            #(#clone_fields,)*
                            _marker: ::core::marker::PhantomData,
                        }
                    }
                }

                impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
                #debug_where_clause
                {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        f.debug_struct(#ast_name_str)#(#debug_fields)*.finish()
                    }
                }

                impl #impl_generics PartialEq for #ast_name #ty_generics
                #where_clause
                {
                    fn eq(&self, other: &Self) -> bool {
                        #eq_comparison
                    }
                }
            }
        }
    }

    /// Generates manual Clone, Debug, PartialEq implementations for wrapper enums.
    ///
    /// Standard #[derive] macros don't work well with GAT projections in enum variants,
    /// so we generate manual implementations with explicit where clauses.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn generate_manual_trait_impls_for_wrapper_enum(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
        data: &kirin_derive_core::ir::DataEnum<ChumskyLayout>,
        ast_name: &syn::Ident,
        ast_generics: &syn::Generics,
        base_bounds: &TokenStream,
        has_parser_bounds: &[TokenStream],
        _has_dialect_parser_bounds: &[TokenStream],
    ) -> TokenStream {
        use kirin_derive_core::ir::VariantRef;
        let crate_path = &self.config.crate_path;
        let ir_type = &self.config.ir_type;

        let (impl_generics, ty_generics, _) = ast_generics.split_for_impl();

        // Collect wrapper types - we only need HasDialectParser bound in base where clause,
        // not the trait-specific bounds (Clone/Debug/PartialEq)
        let wrapper_types = collect_wrapper_types(ir_input);
        let has_dialect_parser_base_bounds: Vec<_> = wrapper_types
            .iter()
            .map(|ty| quote! { #ty: #crate_path::HasDialectParser<'tokens, 'src> })
            .collect();

        // Collect value types that need Debug bounds (types containing type parameters)
        let value_types_needing_bounds = self.collect_value_types_needing_bounds(ir_input);
        let value_debug_bounds: Vec<_> = value_types_needing_bounds
            .iter()
            .map(|ty| quote! { <#ty as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug })
            .collect();

        // Build base where clause without trait-specific bounds
        let where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                #(#has_dialect_parser_base_bounds,)*
        };

        // Debug where clause adds Debug bounds on the actual field types
        let debug_where_clause = quote! {
            where
                #base_bounds
                #(#has_parser_bounds,)*
                #(#has_dialect_parser_base_bounds,)*
                <#ir_type as #crate_path::HasParser<'tokens, 'src>>::Output: ::core::fmt::Debug,
                LanguageOutput: ::core::fmt::Debug,
                #(#value_debug_bounds,)*
        };

        // Collect all variant names and their types for pattern matching.
        // For regular variants, we filter to only include fields that are in the AST
        // (i.e., fields in format string or fields without defaults).
        let variant_arms_clone: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    quote! {
                        #ast_name::#name(inner) => #ast_name::#name(inner.clone())
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = crate::field_kind::collect_fields(stmt);
                    let fields_in_fmt = crate::generate::get_fields_in_format(ir_input, stmt);
                    let filtered = crate::generate::filter_ast_fields(&collected, &fields_in_fmt);

                    if stmt.is_tuple_style() {
                        let fields: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let patterns: Vec<_> = fields.iter().map(|f| quote! { #f }).collect();
                        let clones: Vec<_> = fields.iter().map(|f| quote! { #f.clone() }).collect();
                        quote! {
                            #ast_name::#name(#(#patterns,)*) => #ast_name::#name(#(#clones,)*)
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let clones: Vec<_> = field_names.iter().map(|f| quote! { #f: #f.clone() }).collect();
                        quote! {
                            #ast_name::#name { #(#field_names,)* } => #ast_name::#name { #(#clones,)* }
                        }
                    }
                }
            })
            .collect();

        let variant_arms_debug: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    let name_str = name.to_string();
                    // For wrapper variants, we can't require Debug on the inner GAT type
                    // because that would create circular bounds. Instead, we just print
                    // the variant name without the inner value.
                    quote! {
                        #ast_name::#name(_) => f.debug_tuple(#name_str).field(&"..").finish()
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = crate::field_kind::collect_fields(stmt);
                    let fields_in_fmt = crate::generate::get_fields_in_format(ir_input, stmt);
                    let filtered = crate::generate::filter_ast_fields(&collected, &fields_in_fmt);

                    let name_str = name.to_string();
                    if stmt.is_tuple_style() {
                        let fields: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("f{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let patterns: Vec<_> = fields.iter().map(|f| quote! { #f }).collect();
                        let field_calls: Vec<_> = fields.iter().map(|f| quote! { .field(#f) }).collect();
                        quote! {
                            #ast_name::#name(#(#patterns,)*) => f.debug_tuple(#name_str)#(#field_calls)*.finish()
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let field_calls: Vec<_> = field_names.iter().map(|f| {
                            let name_str = f.to_string();
                            quote! { .field(#name_str, #f) }
                        }).collect();
                        quote! {
                            #ast_name::#name { #(#field_names,)* } => f.debug_struct(#name_str)#(#field_calls)*.finish()
                        }
                    }
                }
            })
            .collect();

        let variant_arms_eq: Vec<TokenStream> = data
            .iter_variants()
            .map(|variant| match variant {
                VariantRef::Wrapper { name, .. } => {
                    quote! {
                        (#ast_name::#name(a), #ast_name::#name(b)) => a == b
                    }
                }
                VariantRef::Regular { name, stmt } => {
                    // Get filtered AST fields (excludes default fields not in format)
                    let collected = crate::field_kind::collect_fields(stmt);
                    let fields_in_fmt = crate::generate::get_fields_in_format(ir_input, stmt);
                    let filtered = crate::generate::filter_ast_fields(&collected, &fields_in_fmt);

                    if stmt.is_tuple_style() {
                        let fields_a: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("a{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let fields_b: Vec<_> = (0..filtered.len())
                            .map(|i| syn::Ident::new(&format!("b{}", i), proc_macro2::Span::call_site()))
                            .collect();
                        let comparisons: Vec<_> = fields_a.iter().zip(fields_b.iter())
                            .map(|(a, b)| quote! { #a == #b })
                            .collect();
                        let comparison = if comparisons.is_empty() {
                            quote! { true }
                        } else {
                            quote! { #(#comparisons)&&* }
                        };
                        quote! {
                            (#ast_name::#name(#(#fields_a,)*), #ast_name::#name(#(#fields_b,)*)) => #comparison
                        }
                    } else {
                        let field_names: Vec<_> = filtered.iter()
                            .filter_map(|f| f.ident.as_ref())
                            .collect();
                        let fields_a: Vec<_> = field_names.iter()
                            .map(|f| syn::Ident::new(&format!("{}_a", f), f.span()))
                            .collect();
                        let fields_b: Vec<_> = field_names.iter()
                            .map(|f| syn::Ident::new(&format!("{}_b", f), f.span()))
                            .collect();
                        let patterns_a: Vec<_> = field_names.iter().zip(fields_a.iter())
                            .map(|(n, a)| quote! { #n: #a })
                            .collect();
                        let patterns_b: Vec<_> = field_names.iter().zip(fields_b.iter())
                            .map(|(n, b)| quote! { #n: #b })
                            .collect();
                        let comparisons: Vec<_> = fields_a.iter().zip(fields_b.iter())
                            .map(|(a, b)| quote! { #a == #b })
                            .collect();
                        let comparison = if comparisons.is_empty() {
                            quote! { true }
                        } else {
                            quote! { #(#comparisons)&&* }
                        };
                        quote! {
                            (#ast_name::#name { #(#patterns_a,)* }, #ast_name::#name { #(#patterns_b,)* }) => #comparison
                        }
                    }
                }
            })
            .collect();

        // Generate additional bounds for traits
        // Clone and PartialEq bounds are needed for wrapper variants.
        // Debug does NOT need bounds because we print a placeholder for wrapper variants.
        let wrapper_types = collect_wrapper_types(ir_input);
        let clone_bounds: Vec<_> = wrapper_types.iter()
            .map(|ty| quote! { <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>: Clone })
            .collect();
        let partial_eq_bounds: Vec<_> = wrapper_types.iter()
            .map(|ty| quote! { <#ty as #crate_path::HasDialectParser<'tokens, 'src>>::Output<TypeOutput, LanguageOutput>: PartialEq })
            .collect();

        quote! {
            impl #impl_generics Clone for #ast_name #ty_generics
            #where_clause
                #(#clone_bounds,)*
            {
                fn clone(&self) -> Self {
                    match self {
                        #(#variant_arms_clone,)*
                        #ast_name::__Marker(_, unreachable) => match *unreachable {},
                    }
                }
            }

            impl #impl_generics ::core::fmt::Debug for #ast_name #ty_generics
            #debug_where_clause
            {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        #(#variant_arms_debug,)*
                        #ast_name::__Marker(_, unreachable) => match *unreachable {},
                    }
                }
            }

            impl #impl_generics PartialEq for #ast_name #ty_generics
            #where_clause
                #(#partial_eq_bounds,)*
            {
                fn eq(&self, other: &Self) -> bool {
                    match (self, other) {
                        #(#variant_arms_eq,)*
                        (#ast_name::__Marker(_, unreachable), _) => match *unreachable {},
                        (_, #ast_name::__Marker(_, unreachable)) => match *unreachable {},
                        _ => false,
                    }
                }
            }
        }
    }
}
