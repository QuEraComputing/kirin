use crate::data::CombineGenerics;

pub struct Builder;

impl CombineGenerics for Builder {
    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics {
        other.clone()
    }
}
