use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub(super) kind: FieldKind,
    pub(super) collection: FieldCollectionKind,
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
pub(super) enum FieldCollectionKind {
    Vec,
    Option,
    None,
}

macro_rules! impl_from_context {
    ($($name:ident),*) => {
        impl<'src, T: Layout> ScanExtra<'src, syn::Field, FieldMeta> for T {
            fn scan_extra(&self, node: &'src syn::Field) -> syn::Result<FieldMeta> {
                let ty = &node.ty;
                let extra = $(
                    if is_type(ty, stringify!($name)) {
                        FieldMeta {
                            kind: FieldKind::$name,
                            collection: FieldCollectionKind::None,
                        }
                    } else if is_type_in(ty, stringify!($name), |seg| seg.ident == "Vec") {
                        FieldMeta {
                            kind: FieldKind::$name,
                            collection: FieldCollectionKind::Vec,
                        }
                    } else if is_type_in(ty, stringify!($name), |seg| seg.ident == "Option") {
                        FieldMeta {
                            kind: FieldKind::$name,
                            collection: FieldCollectionKind::Option,
                        }
                    } else
                )* {
                    FieldMeta {
                        kind: FieldKind::Other,
                        collection: FieldCollectionKind::None,
                    }
                };
                Ok(extra)
            }
        }
    }
}

impl_from_context!(SSAValue, ResultValue, Block, Region, Successor);
