use crate::{DeriveContext, DeriveHelperAttribute};

mod config;
mod trait_impl;
mod iterator;

pub use config::Config;

pub struct FieldAccessor<A: DeriveHelperAttribute> {
    iterator: iterator::IteratorImpl,
    accessor: trait_impl::AccessorImpl,
    _marker: std::marker::PhantomData<A>,
}

impl<A: DeriveHelperAttribute> FieldAccessor<A> {
    pub fn new(
        config: Config,
        ctx: &DeriveContext<A>,
    ) -> Self {
        let accessor = trait_impl::AccessorImpl::new(&config, ctx);
        let iterator = iterator::IteratorImpl::new(&config, ctx);
        Self { iterator, accessor, _marker: std::marker::PhantomData }
    }
}

impl<A: DeriveHelperAttribute> crate::traits::WriteTokenStream for FieldAccessor<A> {
    type HelperAttribute = A;
    fn write_token(&mut self, ctx: &mut DeriveContext<Self::HelperAttribute>) -> eyre::Result<()> {
        ctx.write_trait_impl(self.accessor.generate());
        ctx.write_helper_impl(self.iterator.generate());
        Ok(())
    }
}
