use proc_macro2::TokenStream;

use crate::DeriveContext;

pub trait DeriveHelperAttribute: Clone {
    fn scan(input: &syn::DeriveInput) -> eyre::Result<Self>
    where
        Self: Sized;
    fn global_wraps(&self) -> bool;
    fn variant_wraps(&self, variant: &syn::Ident) -> bool;
}

pub trait WriteTokenStream {
    type HelperAttribute: DeriveHelperAttribute;
    fn write_token(&mut self, ctx: &mut DeriveContext<Self::HelperAttribute>) -> eyre::Result<()>;
}

pub trait DeriveTrait: WriteTokenStream + Sized {
    fn scan(ctx: &DeriveContext<Self::HelperAttribute>) -> eyre::Result<Self>
    where
        Self: Sized;

    fn trait_path() -> TokenStream;
    fn generate(input: syn::DeriveInput) -> TokenStream {
        let mut ctx = DeriveContext::new(Self::trait_path(), input);
        let mut derive_trait = Self::scan(&ctx).unwrap();
        derive_trait.write_token(&mut ctx).unwrap();
        ctx.generate()
    }
}
