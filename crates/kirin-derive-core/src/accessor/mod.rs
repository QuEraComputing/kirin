use crate::{DeriveContext, DeriveHelperAttribute};

mod config;
mod trait_impl;
mod iterator;

pub use config::Config;

pub struct FieldAccessor {
    iterator: iterator::IteratorImpl,
    accessor: trait_impl::AccessorImpl,
}

impl FieldAccessor {
    pub fn new<A: DeriveHelperAttribute>(
        config: Config,
        ctx: &DeriveContext<A>,
    ) -> Self {
        let accessor = trait_impl::AccessorImpl::new(&config, ctx);
        let iterator = iterator::IteratorImpl::new(&config, ctx);
        Self { iterator, accessor }
    }
}

impl<A: DeriveHelperAttribute> crate::traits::Generate<A> for FieldAccessor {
    fn generate(&mut self, ctx: &mut DeriveContext<A>) -> eyre::Result<()> {
        ctx.write_trait_impl(self.accessor.generate());
        ctx.write_helper_impl(self.iterator.generate());
        Ok(())
    }
}
