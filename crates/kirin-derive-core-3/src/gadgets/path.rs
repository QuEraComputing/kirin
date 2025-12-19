use quote::ToTokens;

use crate::{
    derive::{Compile, DeriveTrait, DeriveWithCratePath, WithUserCratePath},
    ir::Attrs,
    target,
};

target! {
    pub struct TraitPath
}

impl<'src, L, T> Compile<'src, T, TraitPath> for L
where
    T: Attrs<Output: WithUserCratePath>,
    L: DeriveTrait + DeriveWithCratePath,
{
    fn compile(&self, node: &T) -> TraitPath {
        let mut new_path = if let Some(path) = &node.attrs().crate_path() {
            path
        } else {
            self.crate_path()
        }
        .clone();
        new_path.segments.extend(self.trait_path().segments.clone());
        TraitPath(new_path.to_token_stream())
    }
}

target! {
    pub struct CratePath
}

impl<'src, L, T> Compile<'src, T, CratePath> for L
where
    T: Attrs<Output: WithUserCratePath>,
    L: DeriveWithCratePath,
{
    fn compile(&self, node: &T) -> CratePath {
        let path = if let Some(path) = &node.attrs().crate_path() {
            path
        } else {
            self.crate_path()
        };
        CratePath(path.to_token_stream())
    }
}
