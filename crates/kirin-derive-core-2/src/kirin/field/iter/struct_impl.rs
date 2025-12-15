use quote::{ToTokens, quote};

use crate::{
    data::{Compile, DialectStruct},
    kirin::field::{
        context::FieldsIter,
        iter::{FieldIterator, field::IteratorTypeDefHead},
    },
};

impl<'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>>
    for IteratorTypeDefStruct<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(IteratorTypeDefStruct {
            head: IteratorTypeDefHead::compile(ctx, node)?,
            field: FieldIterator::compile(ctx, &node.statement)?,
        })
    }
}

pub struct IteratorTypeDefStruct<'a> {
    pub head: IteratorTypeDefHead,
    pub field: FieldIterator<'a>,
}

impl ToTokens for IteratorTypeDefStruct<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let head = &self.head;
        let ty = &self.field.ty();
        quote! {
            #[automatically_derived]
            pub struct #head {
                inner: #ty,
            }
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>> for IteratorImplStruct<'src> {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(IteratorImplStruct {
            iter: IteratorTypeDefStruct::compile(ctx, node)?,
        })
    }
}

pub struct IteratorImplStruct<'a> {
    pub iter: IteratorTypeDefStruct<'a>,
}

impl<'a> IteratorImplStruct<'a> {
    pub fn expr(&self) -> &FieldIterator<'a> {
        &self.iter.field
    }
}

impl ToTokens for IteratorImplStruct<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let iter = &self.iter;
        let item = &self.iter.field.matching_item;
        let head = self.iter.head.impl_head();
        quote! {
            #iter
            #head {
                type Item = #item;
                fn next(&mut self) -> Option<Self::Item> {
                    self.inner.next()
                }
            }
        }
        .to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use crate::data::FromContext;

    use super::*;

    #[test]
    fn test_struct_iterator_type_to_tokens() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = "MyLattice")]
            struct MyStruct<T: Bound> {
                values: Vec<SSAValue>,
                count: SSAValue,
                marker: std::marker::PhantomData<T>,
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
                .build();
            let data = DialectStruct::from_context(&ctx, &input).unwrap();
            let t = syn::parse_file(
                &IteratorImplStruct::compile(&ctx, &data)
                    .unwrap()
                    .into_token_stream()
                    .to_string(),
            )
            .unwrap();
            insta::assert_snapshot!(prettyplease::unparse(&t));
        }
    }

    #[test]
    fn test_generic_struct() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = "MyLattice")]
            struct MyStruct<T: Bound> {
                values: Vec<SSAValue>,
                count: SSAValue,
                param: T,
            }
        };

        for mutable in [false, true] {
            let ctx = FieldsIter::builder()
                .mutable(mutable)
                .trait_lifetime("'a")
                .matching_type("T")
                .default_crate_path("kirin::ir")
                .trait_path("HasParams")
                .trait_method("params")
                .build();
            let data = DialectStruct::from_context(&ctx, &input).unwrap();
            let t = syn::parse_file(
                &IteratorImplStruct::compile(&ctx, &data)
                    .unwrap()
                    .into_token_stream()
                    .to_string(),
            )
            .unwrap();
            insta::assert_snapshot!(prettyplease::unparse(&t));
        }
    }
}
