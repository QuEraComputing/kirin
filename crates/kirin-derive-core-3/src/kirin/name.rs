use crate::prelude::*;

use super::attrs::{KirinEnumOptions, KirinFieldOptions, KirinStructOptions, KirinVariantOptions};

pub struct Name {
    crate_path: syn::Path,
    trait_path: syn::Path,
}

impl Layout for Name {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type FieldExtra = ();
    type StatementExtra = ();
}

impl DeriveTrait for Name {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}

impl DeriveWithCratePath for Name {
    fn default_crate_path(&self) -> &syn::Path {
        &self.crate_path
    }
}

// impl<'src> Compile<'src, Enum<'src, Name>, TokenStream> for Name {
//     fn compile(&self, node: &Enum<'src, Name>) -> TokenStream {
//     }
// }