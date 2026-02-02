use kirin_derive_core::ir::{fields::Collection, DefaultValue};
use kirin_derive_core::prelude::*;
use quote::format_ident;

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) build_fn_name: syn::Ident,
    pub(crate) is_wrapper: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FieldInfo {
    pub(crate) index: usize,
    pub(crate) ident: Option<syn::Ident>,
    pub(crate) kind: FieldKind,
}

#[derive(Clone, Debug)]
pub(crate) enum FieldKind {
    Wrapper {
        ty: syn::Type,
    },
    Argument {
        collection: Collection,
    },
    Result {
        collection: Collection,
        ssa_ty: syn::Expr,
    },
    Block {
        collection: Collection,
    },
    Successor {
        collection: Collection,
    },
    Region {
        collection: Collection,
    },
    Value {
        ty: syn::Type,
        into: bool,
        default: Option<DefaultValue>,
    },
}

impl FieldInfo {
    pub(crate) fn name_ident(&self, fallback: proc_macro2::Span) -> syn::Ident {
        self.ident
            .clone()
            .unwrap_or_else(|| format_ident!("field_{}", self.index, span = fallback))
    }
}

pub(crate) struct StatementBuilder;

impl StatementBuilder {
    pub(crate) fn collect_fields(statement: &ir::Statement<StandardLayout>) -> Vec<FieldInfo> {
        let mut fields = Vec::new();
        if let Some(wrapper) = &statement.wraps {
            fields.push(FieldInfo {
                index: wrapper.field.index,
                ident: wrapper.field.ident.clone(),
                kind: FieldKind::Wrapper {
                    ty: wrapper.ty.clone(),
                },
            });
        }

        for arg in statement.arguments.iter() {
            fields.push(FieldInfo {
                index: arg.field.index,
                ident: arg.field.ident.clone(),
                kind: FieldKind::Argument {
                    collection: arg.collection.clone(),
                },
            });
        }

        for result in statement.results.iter() {
            fields.push(FieldInfo {
                index: result.field.index,
                ident: result.field.ident.clone(),
                kind: FieldKind::Result {
                    collection: result.collection.clone(),
                    ssa_ty: result.ty.clone(),
                },
            });
        }

        for block in statement.blocks.iter() {
            fields.push(FieldInfo {
                index: block.field.index,
                ident: block.field.ident.clone(),
                kind: FieldKind::Block {
                    collection: block.collection.clone(),
                },
            });
        }

        for successor in statement.successors.iter() {
            fields.push(FieldInfo {
                index: successor.field.index,
                ident: successor.field.ident.clone(),
                kind: FieldKind::Successor {
                    collection: successor.collection.clone(),
                },
            });
        }

        for region in statement.regions.iter() {
            fields.push(FieldInfo {
                index: region.field.index,
                ident: region.field.ident.clone(),
                kind: FieldKind::Region {
                    collection: region.collection.clone(),
                },
            });
        }

        for value in statement.values.iter() {
            fields.push(FieldInfo {
                index: value.field.index,
                ident: value.field.ident.clone(),
                kind: FieldKind::Value {
                    ty: value.ty.clone(),
                    into: value.into,
                    default: value.default.clone(),
                },
            });
        }

        fields.sort_by_key(|f| f.index);
        fields
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_fields_sorted() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = L)]
            struct Example {
                b: ResultValue,
                a: SSAValue,
            }
        };
        let input = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let ir::Data::Struct(data) = &input.data else {
            panic!("expected struct");
        };
        let fields = StatementBuilder::collect_fields(&data.0);
        let indices: Vec<_> = fields.iter().map(|f| f.index).collect();
        assert_eq!(indices, vec![0, 1]);
    }
}
