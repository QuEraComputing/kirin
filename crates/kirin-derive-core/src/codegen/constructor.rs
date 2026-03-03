use quote::quote;

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
