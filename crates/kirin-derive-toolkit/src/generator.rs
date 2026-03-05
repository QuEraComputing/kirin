use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::context::DeriveContext;
use crate::ir::Layout;

pub trait Generator<L: Layout> {
    fn generate(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>;
}

pub struct GenerateBuilder<'ir, L: Layout> {
    ctx: DeriveContext<'ir, L>,
    generators: Vec<Box<dyn Generator<L> + 'ir>>,
}

impl<L: Layout> crate::ir::Input<L> {
    pub fn generate(&self) -> GenerateBuilder<'_, L> {
        GenerateBuilder {
            ctx: DeriveContext::new(self),
            generators: Vec::new(),
        }
    }
}

impl<'ir, L: Layout> GenerateBuilder<'ir, L> {
    pub fn with(mut self, generator: impl Generator<L> + 'ir) -> Self {
        self.generators.push(Box::new(generator));
        self
    }

    pub fn emit(self) -> darling::Result<TokenStream> {
        let mut combined = TokenStream::new();
        let mut errors = darling::Error::accumulator();

        for generator in &self.generators {
            errors.handle_in(|| {
                let fragments = generator.generate(&self.ctx)?;
                for fragment in fragments {
                    combined.extend(fragment);
                }
                Ok(())
            });
        }

        errors.finish()?;
        debug_dump(&combined);
        Ok(combined)
    }
}

pub fn debug_dump(tokens: &TokenStream) {
    if std::env::var("KIRIN_EXPAND_DEBUG").is_ok() {
        eprintln!("{}", tokens);
    }
}

impl<L: Layout, F> Generator<L> for F
where
    F: Fn(&DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>>,
{
    fn generate(&self, ctx: &DeriveContext<'_, L>) -> darling::Result<Vec<TokenStream>> {
        self(ctx)
    }
}

impl<L: Layout> ToTokens for DeriveContext<'_, L> {
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}
