use crate::{data::*, target};
use quote::ToTokens;

target! {
    pub struct TraitPath
}

impl<'src, Ctx, T> Compile<'src, T, TraitPath> for Ctx
where
    T: TopLevel<'src, Ctx>,
    Ctx: TraitContext<'src> + AllowCratePath<'src>,
{
    fn compile(&self, node: &T) -> TraitPath {
        let mut new_path = if let Some(path) = &node.attrs_global().crate_path() {
            path
        } else {
            self.crate_path()
        }
        .clone();
        new_path.segments.extend(self.trait_path().segments.clone());
        TraitPath(new_path.to_token_stream())
    }
}
