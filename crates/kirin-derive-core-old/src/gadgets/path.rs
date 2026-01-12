use quote::ToTokens;

use crate::{
    derive::{Compile, DeriveTrait, DeriveWithCratePath},
    ir::WithUserCratePath,
    target,
};

target! {
    pub struct TraitPath
}

impl<'src, L, T> Compile<'src, L, TraitPath> for T
where
    T: WithUserCratePath,
    L: DeriveTrait + DeriveWithCratePath,
{
    fn compile(&self, ctx: &L) -> TraitPath {
        let mut new_path = if let Some(path) = &self.user_crate_path() {
            path
        } else {
            ctx.default_crate_path()
        }
        .clone();
        new_path.segments.extend(ctx.trait_path().segments.clone());
        TraitPath(new_path.to_token_stream())
    }
}

target! {
    pub struct CratePath
}

impl<'src, L, T> Compile<'src, L, CratePath> for T
where
    T: WithUserCratePath,
    L: DeriveWithCratePath,
{
    fn compile(&self, ctx: &L) -> CratePath {
        let path = if let Some(path) = &self.user_crate_path() {
            path
        } else {
            ctx.default_crate_path()
        };
        CratePath(path.to_token_stream())
    }
}
