use quote::quote;

use super::impl_head::ImplHead;
use super::type_head::TypeHead;
use super::variant::VariantTypeDef;
use crate::prelude::*;
use crate::kirin::field::context::FieldsIter;
use super::item::MatchingItem;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, Enum<'src, Self>, EnumImpl> for FieldsIter {
    fn compile(&self, node: &Enum<'src, Self>) -> EnumImpl {
        let head: ImplHead = self.compile(node);
        let item: MatchingItem = self.compile(node);
        let iter: EnumTypeDef = self.compile(node);
        let variant_names = node
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

impl<'src> Compile<'src, Enum<'src, Self>, EnumTypeDef> for FieldsIter {
    fn compile(&self, node: &Enum<'src, Self>) -> EnumTypeDef {
        let head: TypeHead = self.compile(node);
        let variants = node
            .variants()
            .map(|v| self.compile(&v))
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
