use quote::quote;

use crate::ir::{Layout, fields::FieldInfo};

/// Builds constructor expressions for structs or enum variants.
///
/// Handles both named (`Foo { a, b }`) and tuple (`Foo(a, b)`) styles,
/// mapping each field through a user-provided closure.
///
/// ```ignore
/// let ctor = ConstructorBuilder::new_struct(&type_name, is_tuple);
/// let tokens = ctor.build(&stmt.fields, |field| {
///     let name = field.name_ident(Span::call_site());
///     quote!(#name)
/// });
/// ```
pub struct ConstructorBuilder<'a> {
    type_name: &'a syn::Ident,
    variant_name: Option<&'a syn::Ident>,
    is_tuple: bool,
}

impl<'a> ConstructorBuilder<'a> {
    /// Create a builder for a struct constructor expression.
    pub fn new_struct(type_name: &'a syn::Ident, is_tuple: bool) -> Self {
        Self {
            type_name,
            variant_name: None,
            is_tuple,
        }
    }

    /// Create a builder for an enum variant constructor expression.
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

    /// Emit a constructor using the concrete type name (e.g., `MyOp { ... }`).
    ///
    /// Calls `value_fn` for each field to produce the value expression.
    pub fn build<L, F>(&self, fields: &[FieldInfo<L>], value_fn: F) -> proc_macro2::TokenStream
    where
        L: Layout,
        F: Fn(&FieldInfo<L>) -> proc_macro2::TokenStream,
    {
        let type_name = self.type_name;
        let field_values: Vec<_> = fields.iter().map(&value_fn).collect();

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

    /// Emit a constructor using `Self` (e.g., `Self::Variant { ... }`).
    ///
    /// Same as [`build`](Self::build) but uses `Self` instead of the type name.
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
