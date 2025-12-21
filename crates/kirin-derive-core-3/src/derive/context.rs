use crate::ir::*;

pub trait DeriveWithCratePath:
    Layout<StructAttr: WithUserCratePath, EnumAttr: WithUserCratePath>
{
    /// get the default crate path to use for the derivation
    /// if the derive macro allows specifying a crate path via global
    /// attribute, this will be overridden
    fn default_crate_path(&self) -> &syn::Path;
}

/// A derive implementation context for a specific trait derivation
/// this context must provide an option to specify the crate path
/// of the paired runtime crate that contains the trait being derived
pub trait DeriveTrait: DeriveWithCratePath {
    /// get the relative path to the trait being implemented
    /// the relative path is relative to the crate path
    /// either specified by the user or defaulted
    fn trait_path(&self) -> &syn::Path;
}

pub trait DeriveTraitWithGenerics: DeriveTrait {
    /// get the generics of the type being derived
    fn generics(&self) -> &syn::Generics;

    /// combine the current generics with another set of generics
    fn combine_generics(&self, other: &syn::Generics) -> syn::Generics {
        let mut combined = self.generics().clone();
        combined.params.extend(other.params.clone());
        combined
    }

    /// add a lifetime parameter to the generics
    fn add_lifetime(&self, lifetime: syn::Lifetime) -> syn::Generics {
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                lifetime,
            )));
        self.combine_generics(&generics)
    }
}
