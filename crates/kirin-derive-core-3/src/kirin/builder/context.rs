use crate::{
    prelude::*,
    kirin::attrs::{KirinEnumOptions, KirinFieldOptions, KirinStructOptions, KirinVariantOptions},
};

pub struct Builder(syn::Path);

impl Default for Builder {
    fn default() -> Self {
        Builder(syn::parse_quote! { ::kirin::ir })
    }
}

impl Layout for Builder {
    type EnumAttr = KirinEnumOptions;
    type StructAttr = KirinStructOptions;
    type VariantAttr = KirinVariantOptions;
    type FieldAttr = KirinFieldOptions;
    type FieldExtra = FieldExtra;
    type StatementExtra = ();
}

impl DeriveWithCratePath for Builder {
    fn crate_path(&self) -> &syn::Path {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct FieldExtra {
    pub(super) kind: FieldKind,
    pub(super) container: FieldContainerKind,
}

#[derive(Debug, Clone)]
pub(super) enum FieldKind {
    SSAValue,
    ResultValue,
    Block,
    Region,
    Successor,
    Other,
}

#[derive(Debug, Clone)]
pub(super) enum FieldContainerKind {
    Vec,
    Option,
    None,
}

macro_rules! impl_from_context {
    ($($name:ident),*) => {
        impl<'src> ScanExtra<'src, syn::Field, FieldExtra> for Builder {
            fn scan_extra(&self, node: &'src syn::Field) -> syn::Result<FieldExtra> {
                let ty = &node.ty;
                let extra = $(
                    if is_type(ty, stringify!($name)) {
                        FieldExtra {
                            kind: FieldKind::$name,
                            container: FieldContainerKind::None,
                        }
                    } else if is_type_in(ty, stringify!($name), |seg| seg.ident == "Vec") {
                        FieldExtra {
                            kind: FieldKind::$name,
                            container: FieldContainerKind::Vec,
                        }
                    } else if is_type_in(ty, stringify!($name), |seg| seg.ident == "Option") {
                        FieldExtra {
                            kind: FieldKind::$name,
                            container: FieldContainerKind::Option,
                        }
                    } else
                )* {
                    FieldExtra {
                        kind: FieldKind::Other,
                        container: FieldContainerKind::None,
                    }
                };
                Ok(extra)
            }
        }
    }
}

impl_from_context!(SSAValue, ResultValue, Block, Region, Successor);
