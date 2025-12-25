use crate::chumsky::attrs::*;
use crate::{
    kirin::extra::{FieldMeta},
    prelude::*,
};
use bon::Builder;

#[derive(Clone, Builder)]
pub struct DeriveHasParser {
    #[builder(default = syn::parse_quote!(kirin::parsers))]
    pub default_crate_path: syn::Path,
    #[builder(default = syn::parse_quote!(HasParser))]
    pub trait_path: syn::Path,
}

impl Layout for DeriveHasParser {
    type EnumAttr = ChumskyEnumOptions;
    type StructAttr = ChumskyStructOptions;
    type VariantAttr = ChumskyVariantOptions;
    type FieldAttr = ();
    type FieldExtra = FieldMeta;
    type StatementExtra = ();
}

impl DeriveWithCratePath for DeriveHasParser {
    fn default_crate_path(&self) -> &syn::Path {
        &self.default_crate_path
    }
}

impl DeriveTrait for DeriveHasParser {
    fn trait_path(&self) -> &syn::Path {
        &self.trait_path
    }
}
