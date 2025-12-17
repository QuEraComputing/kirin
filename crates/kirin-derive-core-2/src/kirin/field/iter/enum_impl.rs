use quote::quote;

use super::impl_head::ImplHead;
use super::type_head::TypeHead;
use super::variant::VariantTypeDef;
use crate::data::*;
use crate::kirin::field::FieldsIter;
use crate::kirin::field::iter::item::MatchingItem;
use crate::target;

target! {
    pub struct EnumImpl
}

impl<'src> Compile<'src, DialectEnum<'src, Self>, EnumImpl> for FieldsIter {
    fn compile(&self, node: &DialectEnum<'src, Self>) -> EnumImpl {
        let head: ImplHead = self.compile(node);
        let item: MatchingItem = self.compile(node);
        let iter: EnumTypeDef = self.compile(node);
        let variant_names = node
            .variants
            .iter()
            .map(|variant| variant.source_ident())
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

impl<'src> Compile<'src, DialectEnum<'src, Self>, EnumTypeDef> for FieldsIter {
    fn compile(&self, node: &DialectEnum<'src, Self>) -> EnumTypeDef {
        let head: TypeHead = self.compile(node);
        let variants = node
            .variants
            .iter()
            .map(|variant| self.compile(variant))
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
