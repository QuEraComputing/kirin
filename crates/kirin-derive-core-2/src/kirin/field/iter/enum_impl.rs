use super::field::MatchingItem;
use super::variant::IteratorTypeDefVariant;
use crate::kirin::field::context::FieldsIter;
use crate::{data::*, kirin::field::iter::field::IteratorTypeDefHead};

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

impl<'src> Compile<'src, FieldsIter, DialectEnum<'src, FieldsIter>> for EnumIteratorTypeDef<'src> {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectEnum<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        let variants = node
            .variants
            .iter()
            .map(|variant| IteratorTypeDefVariant::compile(ctx, variant))
            .collect::<syn::Result<Vec<_>>>()?;

        Ok(EnumIteratorTypeDef {
            head: IteratorTypeDefHead::compile(ctx, node)?,
            variant: variants,
        })
    }
}

/// Definition of the enum iterator type
pub struct EnumIteratorTypeDef<'src> {
    head: IteratorTypeDefHead,
    variant: Vec<IteratorTypeDefVariant<'src>>,
}

impl EnumIteratorTypeDef<'_> {
    pub fn ty(&self) -> TokenStream {
        self.head.ty()
    }
}

impl<'a> ToTokens for EnumIteratorTypeDef<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let head = &self.head;
        let variants = &self.variant;
        quote! {
            #[automatically_derived]
            pub enum #head {
                #(#variants),*
            }
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, DialectEnum<'src, FieldsIter>> for IteratorImplEnum<'src> {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectEnum<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        let syn::Data::Enum(src) = &node.input().data else {
            return Err(syn::Error::new_spanned(
                node.input(),
                "EnumIteratorImpl can only be created from enum data",
            ));
        };

        Ok(IteratorImplEnum {
            src,
            iter: EnumIteratorTypeDef::compile(ctx, node)?,
            matching_item: MatchingItem::builder()
                .lifetime(&ctx.trait_lifetime)
                .matching_type(&ctx.matching_type)
                .mutable(ctx.mutable)
                .build(),
        })
    }
}

#[derive(bon::Builder)]
pub struct IteratorImplEnum<'a> {
    src: &'a syn::DataEnum,
    iter: EnumIteratorTypeDef<'a>,
    matching_item: MatchingItem<'a>,
}

impl IteratorImplEnum<'_> {
    /// return the iterator type with generics applied (without bounds)
    /// e.g `<IterName><'trait_lifetime, ...>`
    /// assuming the generics have been set up correctly
    pub fn ty(&self) -> TokenStream {
        self.iter.ty()
    }
}

impl<'a> ToTokens for IteratorImplEnum<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let iter = &self.iter;
        let head = self.iter.head.impl_head();
        let item = &self.matching_item;
        let variant_names = self.src.variants.iter().map(|v| &v.ident);
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
        .to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_iterator_type_to_tokens() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = "MyLattice")]
            enum MyEnum<T> {
                A(Vec<SSAValue>, SSAValue),
                B(SSAValue, SSAValue, T),
            }
        };

        let ctx = FieldsIter::builder()
            .mutable(false)
            .trait_lifetime("'a")
            .matching_type("SSAValue")
            .default_crate_path("kirin::ir")
            .trait_path("HasArguments")
            .trait_method("arguments")
            .trait_type_iter("Iter")
            .build();
        let data = DialectEnum::from_context(&ctx, &input).unwrap();
        let t = syn::parse_file(
            &EnumIteratorTypeDef::compile(&ctx, &data)
                .unwrap()
                .into_token_stream()
                .to_string(),
        )
        .unwrap();
        insta::assert_snapshot!(prettyplease::unparse(&t));
    }

    #[test]
    fn test_enum_iterator_impl_to_tokens() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = "MyLattice")]
            enum MyEnum<T> {
                A(Vec<SSAValue>, SSAValue),
                B(SSAValue, SSAValue, T),
                C(T),
            }
        };

        for mutable in [false, true] {
            let ctx = FieldsIter::builder()
                .mutable(mutable)
                .trait_lifetime("'a")
                .matching_type("SSAValue")
                .default_crate_path("kirin::ir")
                .trait_path("HasArguments")
                .trait_method("arguments")
                .trait_type_iter("Iter")
                .build();
            let data = DialectEnum::from_context(&ctx, &input).unwrap();
            let t = syn::parse_file(
                &IteratorImplEnum::compile(&ctx, &data)
                    .unwrap()
                    .into_token_stream()
                    .to_string(),
            )
            .unwrap();
            insta::assert_snapshot!(prettyplease::unparse(&t));
        }
    }
}
