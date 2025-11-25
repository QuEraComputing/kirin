use crate::data::{CombineGenerics, HasDefaultCratePath};

pub struct Builder;

impl CombineGenerics for Builder {
    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics {
        other.clone()
    }
}

impl HasDefaultCratePath for Builder {
    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }
}
