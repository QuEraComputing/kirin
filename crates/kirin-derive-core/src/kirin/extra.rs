use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub kind: FieldKind,
    pub collection: FieldCollectionKind,
}

#[derive(Debug, Clone)]
pub enum FieldKind {
    SSAValue,
    ResultValue,
    Block,
    Region,
    Successor,
    Other,
}

impl std::fmt::Display for FieldKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldKind::SSAValue => write!(f, "SSAValue"),
            FieldKind::ResultValue => write!(f, "ResultValue"),
            FieldKind::Block => write!(f, "Block"),
            FieldKind::Region => write!(f, "Region"),
            FieldKind::Successor => write!(f, "Successor"),
            FieldKind::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FieldCollectionKind {
    Vec,
    Option,
    Single,
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
                            collection: FieldCollectionKind::Single,
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
                        collection: FieldCollectionKind::Single,
                    }
                };
                Ok(extra)
            }
        }
    }
}

impl_from_context!(SSAValue, ResultValue, Block, Region, Successor);
