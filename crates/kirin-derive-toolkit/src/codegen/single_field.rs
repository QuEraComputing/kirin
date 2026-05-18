use proc_macro2::TokenStream;
use quote::quote;

#[derive(Debug, Clone)]
pub struct SingleField {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
}

impl SingleField {
    pub fn from_fields(fields: &syn::Fields) -> syn::Result<Self> {
        if fields.len() != 1 {
            return Err(syn::Error::new_spanned(
                fields,
                "derivation only supports variants with exactly one field",
            ));
        }
        let field = fields.iter().next().unwrap();
        Ok(Self {
            ident: field.ident.clone(),
            ty: field.ty.clone(),
        })
    }

    pub fn constructor(&self, binding: &syn::Ident) -> TokenStream {
        if let Some(field_name) = &self.ident {
            quote! { { #field_name: #binding } }
        } else {
            quote! { (#binding) }
        }
    }

    pub fn pattern(&self, binding: &syn::Ident) -> TokenStream {
        if let Some(field_name) = &self.ident {
            quote! { { #field_name: #binding } }
        } else {
            quote! { (#binding) }
        }
    }
}
