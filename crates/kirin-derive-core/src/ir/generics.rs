use super::{Enum, Layout, Source, Struct};

pub trait WithGenerics {
    fn generics(&self) -> &syn::Generics;
    fn add_lifetime(&self, lifetime: syn::Lifetime) -> syn::Generics {
        let mut generics = self.generics().clone();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                lifetime,
            )));
        generics
    }
}

impl<'src, L: Layout> WithGenerics for Struct<'src, L> {
    fn generics(&self) -> &syn::Generics {
        &self.source().generics
    }
}

impl<'src, L: Layout> WithGenerics for Enum<'src, L> {
    fn generics(&self) -> &syn::Generics {
        &self.source().generics
    }
}
