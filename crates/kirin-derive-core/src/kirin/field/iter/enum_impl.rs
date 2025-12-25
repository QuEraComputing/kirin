use quote::quote;

use super::impl_head::ImplHead;
use super::item::MatchingItem;
use super::type_head::TypeHead;
use super::variant::VariantTypeDef;
use crate::kirin::field::context::FieldsIter;
use crate::prelude::*;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, FieldsIter, EnumImpl> for Enum<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> EnumImpl {
        let head: ImplHead = self.compile(ctx);
        let item: MatchingItem = self.compile(ctx);
        let iter: EnumTypeDef = self.compile(ctx);
        let variant_names = self
            .variants()
            .map(|v| v.source_ident())
            .collect::<Vec<_>>();

        quote! {
            #iter
            #head {
                type Item = #item;
                fn next(&mut self) -> Option<Self::Item> {
                    match self {
                        #(Self::#variant_names(inner) => inner.next(),)*
                    }
                }
            }
        }
        .into()
    }
}

target! {
    pub struct EnumTypeDef
}

impl<'src> Compile<'src, FieldsIter, EnumTypeDef> for Enum<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> EnumTypeDef {
        let head: TypeHead = self.compile(ctx);
        let variants = self
            .variants()
            .map(|v| v.compile(ctx))
            .collect::<Vec<VariantTypeDef>>();

        quote! {
            #[automatically_derived]
            pub enum #head {
                #(#variants),*
            }
        }
        .into()
    }
}
