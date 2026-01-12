use crate::{
    kirin::attrs::{KirinEnumOptions, KirinFieldOptions, KirinStructOptions, KirinVariantOptions},
    prelude::*,
};

use crate::kirin::extra::FieldMeta;
use super::{enum_impl::EnumImpl, struct_impl::StructImpl};

pub struct Builder {
    crate_path: syn::Path,
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            crate_path: syn::parse_quote! { ::kirin::ir },
        }
    }
}

impl Layout for Builder {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type FieldExtra = FieldMeta;
    type StatementExtra = ();
}

impl DeriveWithCratePath for Builder {
    fn default_crate_path(&self) -> &syn::Path {
        &self.crate_path
    }
}

impl Emit<'_> for Builder {
    type EnumImpl = EnumImpl;
    type StructImpl = StructImpl;
}
