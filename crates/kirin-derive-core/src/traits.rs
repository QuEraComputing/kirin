use crate::DeriveContext;

pub trait DeriveHelperAttribute: Clone {
    fn scan(input: &syn::DeriveInput) -> eyre::Result<Self>
    where
        Self: Sized;
    fn global_wraps(&self) -> bool;
    fn variant_wraps(&self, variant: &syn::Ident) -> bool;
}

pub trait Generate<A: DeriveHelperAttribute> {
    fn generate(&mut self, ctx: &mut DeriveContext<A>) -> eyre::Result<()>;
}
