//! Code generation utilities for derive macros.
//!
//! This module provides common helpers for generating code patterns
//! that are frequently needed in derive macro implementations.

use proc_macro2::Span;
use quote::quote;

/// Generates a sequence of identifiers for tuple fields.
///
/// Given a prefix and count, generates identifiers like `f0`, `f1`, `f2`, etc.
fn tuple_field_idents(prefix: &str, count: usize) -> Vec<syn::Ident> {
    (0..count)
        .map(|i| syn::Ident::new(&format!("{}{}", prefix, i), Span::call_site()))
        .collect()
}

/// Generates renamed identifiers from named fields.
///
/// Given a prefix and a list of field identifiers, generates renamed versions
/// like `s_field1`, `s_field2`, etc.
fn renamed_field_idents(prefix: &str, fields: &[syn::Ident]) -> Vec<syn::Ident> {
    fields
        .iter()
        .map(|f| syn::Ident::new(&format!("{}{}", prefix, f), Span::call_site()))
        .collect()
}

/// Field binding information for code generation.
///
/// This struct captures all the identifiers and patterns needed to work with
/// struct/variant fields in generated code.
#[derive(Debug, Clone)]
pub struct FieldBindings {
    /// Whether this is a tuple-style (positional) or named struct/variant.
    pub is_tuple: bool,
    /// The field count (for tuple-style).
    pub field_count: usize,
    /// The field identifiers to use in patterns and expressions.
    /// For tuple-style: generated names like `f0`, `f1`.
    /// For named-style: generated names like `f_fieldname`.
    pub field_idents: Vec<syn::Ident>,
    /// The original field names (for named-style only).
    /// Used to generate patterns like `{ field_name: binding_name }`.
    pub original_field_names: Vec<syn::Ident>,
}

impl FieldBindings {
    /// Creates field bindings for a tuple-style struct/variant.
    pub fn tuple(prefix: &str, count: usize) -> Self {
        Self {
            is_tuple: true,
            field_count: count,
            field_idents: tuple_field_idents(prefix, count),
            original_field_names: Vec::new(),
        }
    }

    /// Creates field bindings for a named struct/variant.
    ///
    /// Generates binding names with the given prefix (e.g., `f_fieldname`).
    pub fn named(prefix: &str, fields: Vec<syn::Ident>) -> Self {
        let count = fields.len();
        let prefixed = renamed_field_idents(&format!("{}_", prefix), &fields);
        Self {
            is_tuple: false,
            field_count: count,
            field_idents: prefixed,
            original_field_names: fields,
        }
    }

    /// Returns true if there are no fields.
    pub fn is_empty(&self) -> bool {
        self.field_count == 0
    }

    /// Generates renamed identifiers with the given prefix.
    ///
    /// For tuple-style, generates `prefix0`, `prefix1`, etc.
    /// For named-style, generates `prefix_fieldname` for each field.
    pub fn renamed(&self, prefix: &str) -> Vec<syn::Ident> {
        if self.is_tuple {
            tuple_field_idents(prefix, self.field_count)
        } else {
            renamed_field_idents(&format!("{}_", prefix), &self.original_field_names)
        }
    }

    /// Creates a new FieldBindings with renamed identifiers.
    ///
    /// This is useful when you need a second set of bindings (e.g., for PartialEq).
    pub fn with_prefix(&self, prefix: &str) -> Self {
        Self {
            is_tuple: self.is_tuple,
            field_count: self.field_count,
            field_idents: self.renamed(prefix),
            original_field_names: self.original_field_names.clone(),
        }
    }
}

// =============================================================================
// Where Clause Utilities
// =============================================================================

/// Combines two optional where clauses into one.
///
/// This is a common pattern when building impls that need to combine
/// the original type's where clause with additional generated bounds.
///
/// # Example
///
/// ```ignore
/// let combined = combine_where_clauses(orig_where.as_ref(), impl_where.as_ref());
/// ```
pub fn combine_where_clauses(
    a: Option<&syn::WhereClause>,
    b: Option<&syn::WhereClause>,
) -> Option<syn::WhereClause> {
    match (a, b) {
        (Some(orig), Some(other)) => {
            let mut combined = orig.clone();
            combined.predicates.extend(other.predicates.iter().cloned());
            Some(combined)
        }
        (Some(wc), None) | (None, Some(wc)) => Some(wc.clone()),
        (None, None) => None,
    }
}

// =============================================================================
// Type Utilities
// =============================================================================

/// Deduplicates a list of types by their token representation.
///
/// This is useful when collecting types for trait bounds, where the same
/// type might appear multiple times from different fields.
///
/// # Example
///
/// ```ignore
/// let mut types = vec![parse_quote!(T), parse_quote!(U), parse_quote!(T)];
/// deduplicate_types(&mut types);
/// // types is now [T, U]
/// ```
pub fn deduplicate_types(types: &mut Vec<syn::Type>) {
    let mut seen = std::collections::HashSet::new();
    types.retain(|ty| {
        let key = quote!(#ty).to_string();
        seen.insert(key)
    });
}

// =============================================================================
// Generics Utilities
// =============================================================================

/// Builder for generics with common lifetime and type parameters.
///
/// This builder helps construct generic parameters for generated types,
/// particularly for AST types that need `'tokens`, `'src`, and `Language` parameters.
pub struct GenericsBuilder<'a> {
    ir_path: &'a syn::Path,
}

impl<'a> GenericsBuilder<'a> {
    /// Creates a new generics builder.
    ///
    /// The `ir_path` is used for adding bounds like `Language: ir_path::Dialect`.
    pub fn new(ir_path: &'a syn::Path) -> Self {
        Self { ir_path }
    }

    /// Adds `'tokens` and `'src: 'tokens` lifetimes to the generics.
    ///
    /// This is useful for types that work with token streams and source references.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = GenericsBuilder::new(&ir_path);
    /// let generics = builder.with_lifetimes(&base_generics);
    /// // generics now has <'tokens, 'src: 'tokens, ...original params...>
    /// ```
    pub fn with_lifetimes(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = base.clone();

        // Add 'tokens lifetime at the beginning if not present
        let tokens_lt = syn::Lifetime::new("'tokens", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "tokens"))
        {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(tokens_lt.clone())),
            );
        }

        // Add 'src: 'tokens lifetime after 'tokens if not present
        let src_lt = syn::Lifetime::new("'src", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Lifetime(l) if l.lifetime.ident == "src"))
        {
            let mut src_param = syn::LifetimeParam::new(src_lt);
            src_param.bounds.push(tokens_lt);
            generics
                .params
                .insert(1, syn::GenericParam::Lifetime(src_param));
        }

        generics
    }

    /// Adds lifetimes and a `Language: Dialect` type parameter.
    ///
    /// This is used for AST types and their trait implementations where
    /// `Language` only needs the `Dialect` bound.
    pub fn with_language(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);
        let ir_path = self.ir_path;

        // Add Language type parameter if not present
        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let mut lang_param = syn::TypeParam::from(lang_ident);
            lang_param.bounds.push(syn::parse_quote!(#ir_path::Dialect));
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }

    /// Adds lifetimes and a `Language` type parameter without bounds.
    ///
    /// This is used when the `Language: Dialect` bound should be specified
    /// in the where clause instead of on the type parameter.
    pub fn with_language_unbounded(&self, base: &syn::Generics) -> syn::Generics {
        let mut generics = self.with_lifetimes(base);

        // Add Language type parameter without any bounds
        let lang_ident = syn::Ident::new("Language", Span::call_site());
        if !generics
            .params
            .iter()
            .any(|p| matches!(p, syn::GenericParam::Type(t) if t.ident == lang_ident))
        {
            let lang_param = syn::TypeParam::from(lang_ident);
            generics.params.push(syn::GenericParam::Type(lang_param));
        }

        generics
    }
}

// =============================================================================
// Constructor Builder
// =============================================================================

use crate::ir::{Layout, fields::FieldInfo};

/// Builder for generating struct/enum variant constructor expressions.
///
/// This builder generates code like:
/// - Named: `TypeName { field1: val1, field2: val2 }`
/// - Tuple: `TypeName(val1, val2)`
/// - Enum variant named: `TypeName::Variant { field1: val1 }`
/// - Enum variant tuple: `TypeName::Variant(val1, val2)`
pub struct ConstructorBuilder<'a> {
    /// The type name (struct name or enum name).
    type_name: &'a syn::Ident,
    /// The variant name for enum variants, None for structs.
    variant_name: Option<&'a syn::Ident>,
    /// Whether this is a tuple-style constructor.
    is_tuple: bool,
}

impl<'a> ConstructorBuilder<'a> {
    /// Creates a new constructor builder for a struct.
    pub fn new_struct(type_name: &'a syn::Ident, is_tuple: bool) -> Self {
        Self {
            type_name,
            variant_name: None,
            is_tuple,
        }
    }

    /// Creates a new constructor builder for an enum variant.
    pub fn new_variant(
        type_name: &'a syn::Ident,
        variant_name: &'a syn::Ident,
        is_tuple: bool,
    ) -> Self {
        Self {
            type_name,
            variant_name: Some(variant_name),
            is_tuple,
        }
    }

    /// Builds the constructor expression.
    ///
    /// The `value_fn` closure is called for each field and should return the
    /// TokenStream representing the value to assign to that field.
    ///
    /// Fields should be provided in declaration order (sorted by index).
    pub fn build<L, F>(&self, fields: &[FieldInfo<L>], value_fn: F) -> proc_macro2::TokenStream
    where
        L: Layout,
        F: Fn(&FieldInfo<L>) -> proc_macro2::TokenStream,
    {
        let type_name = self.type_name;
        let field_values: Vec<_> = fields.iter().map(&value_fn).collect();

        // Handle unit variants (no fields)
        if fields.is_empty() {
            return match self.variant_name {
                Some(variant) => quote! { #type_name::#variant },
                None => quote! { #type_name },
            };
        }

        if self.is_tuple {
            match self.variant_name {
                Some(variant) => quote! { #type_name::#variant(#(#field_values),*) },
                None => quote! { #type_name(#(#field_values),*) },
            }
        } else {
            let field_assigns: Vec<_> = fields
                .iter()
                .zip(field_values.iter())
                .map(|(field, value)| {
                    let name = field.ident.as_ref().expect("named field must have ident");
                    quote! { #name: #value }
                })
                .collect();

            match self.variant_name {
                Some(variant) => quote! { #type_name::#variant { #(#field_assigns),* } },
                None => quote! { #type_name { #(#field_assigns),* } },
            }
        }
    }

    /// Builds the constructor using Self instead of the type name.
    ///
    /// Useful when generating code inside an impl block.
    pub fn build_with_self<L, F>(
        &self,
        fields: &[FieldInfo<L>],
        value_fn: F,
    ) -> proc_macro2::TokenStream
    where
        L: Layout,
        F: Fn(&FieldInfo<L>) -> proc_macro2::TokenStream,
    {
        let field_values: Vec<_> = fields.iter().map(&value_fn).collect();

        // Handle unit variants (no fields)
        if fields.is_empty() {
            return match self.variant_name {
                Some(variant) => quote! { Self::#variant },
                None => quote! { Self },
            };
        }

        if self.is_tuple {
            match self.variant_name {
                Some(variant) => quote! { Self::#variant(#(#field_values),*) },
                None => quote! { Self(#(#field_values),*) },
            }
        } else {
            let field_assigns: Vec<_> = fields
                .iter()
                .zip(field_values.iter())
                .map(|(field, value)| {
                    let name = field.ident.as_ref().expect("named field must have ident");
                    quote! { #name: #value }
                })
                .collect();

            match self.variant_name {
                Some(variant) => quote! { Self::#variant { #(#field_assigns),* } },
                None => quote! { Self { #(#field_assigns),* } },
            }
        }
    }
}
