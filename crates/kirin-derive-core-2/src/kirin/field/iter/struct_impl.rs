use quote::{ToTokens, quote};

use crate::{
    data::{Compile, DialectStruct, FieldMember},
    kirin::{
        attrs::KirinFieldOptions,
        field::{
            context::FieldsIter,
            extra::FieldExtra,
            iter::{
                FieldIterator,
                field::{IteratorTypeDefHead, MatchingItem},
            },
        },
    },
};

pub enum IteratorTypeDefStruct<'a, 'src> {
    Regular(IteratorTypeDefStructRegular<'src>),
    Wrapper(IteratorTypeDefStructWrapper<'a, 'src>),
}

impl<'a, 'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>>
    for IteratorTypeDefStruct<'a, 'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        if node.wraps {
            Ok(IteratorTypeDefStruct::Wrapper(
                IteratorTypeDefStructWrapper::compile(ctx, node)?,
            ))
        } else {
            Ok(IteratorTypeDefStruct::Regular(
                IteratorTypeDefStructRegular::compile(ctx, node)?,
            ))
        }
    }
}

impl<'a, 'src> ToTokens for IteratorTypeDefStruct<'a, 'src> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            IteratorTypeDefStruct::Regular(r) => r.to_tokens(tokens),
            IteratorTypeDefStruct::Wrapper(w) => w.to_tokens(tokens),
        }
    }
}

pub struct IteratorTypeDefStructWrapper<'a, 'src> {
    head: IteratorTypeDefHead,
    wrapper: FieldMember<'a, 'src, KirinFieldOptions, FieldExtra>,
    trait_path: &'src syn::Path,
    trait_method: &'src syn::Ident,
    trait_type_iter: &'src syn::Ident,
}

impl IteratorTypeDefStructWrapper<'_, '_> {
    pub fn expr(&self) -> IteratorExprStructWrapper<'_, '_> {
        IteratorExprStructWrapper(self)
    }
}

impl<'a, 'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>>
    for IteratorTypeDefStructWrapper<'a, 'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        let field = node.statement.fields.wrapper().ok_or_else(|| {
            syn::Error::new_spanned(
                node.input(),
                "Expected exactly one field for wrapper struct",
            )
        })?;

        Ok(IteratorTypeDefStructWrapper {
            head: IteratorTypeDefHead::compile(ctx, node)?,
            wrapper: field,
            trait_path: &ctx.trait_path,
            trait_method: &ctx.trait_method,
            trait_type_iter: &ctx.trait_type_iter,
        })
    }
}

impl<'a, 'src> ToTokens for IteratorTypeDefStructWrapper<'a, 'src> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let head = &self.head;
        let wrapped_type = &self.wrapper.src.ty;
        let trait_path = &self.trait_path;
        let trait_type_iter = &self.trait_type_iter;
        quote! {
            #[automatically_derived]
            pub struct #head {
                inner: <#wrapped_type as #trait_path>::#trait_type_iter,
            }
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>>
    for IteratorTypeDefStructRegular<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        if node.wraps {
            return Err(syn::Error::new_spanned(
                node.input(),
                "Cannot compile regular iterator for a wrapper struct",
            ));
        }

        Ok(IteratorTypeDefStructRegular {
            head: IteratorTypeDefHead::compile(ctx, node)?,
            field: FieldIterator::compile(ctx, &node.statement)?,
        })
    }
}

pub struct IteratorTypeDefStructRegular<'a> {
    pub head: IteratorTypeDefHead,
    pub field: FieldIterator<'a>,
}

impl IteratorTypeDefStructRegular<'_> {
    pub fn expr(&self) -> IteratorExprStructRegular<'_> {
        IteratorExprStructRegular(self)
    }
}

impl ToTokens for IteratorTypeDefStructRegular<'_> {
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

impl<'a, 'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>>
    for IteratorImplStruct<'a, 'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(IteratorImplStruct {
            head: IteratorTypeDefHead::compile(ctx, node)?,
            iter: IteratorTypeDefStruct::compile(ctx, node)?,
            item: MatchingItem::compile(ctx, node)?,
        })
    }
}

pub struct IteratorExprStructRegular<'a>(&'a IteratorTypeDefStructRegular<'a>);

impl ToTokens for IteratorExprStructRegular<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let head = &self.0.head;
        let field = &self.0.field;
        let iter_name = &head.iter_name;
        quote! {
            #iter_name {
                inner: #field,
            }
        }
        .to_tokens(tokens);
    }
}

pub struct IteratorExprStructWrapper<'a, 'src>(&'a IteratorTypeDefStructWrapper<'a, 'src>);

impl ToTokens for IteratorExprStructWrapper<'_, '_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let head = &self.0.head;
        let wrapper = &self.0.wrapper;
        let wrapped_ty = &wrapper.src.ty;
        let trait_path = &self.0.trait_path;
        let trait_method = &self.0.trait_method;
        let iter_name = &head.iter_name;
        quote! {
            #iter_name {
                inner: <#wrapped_ty as #trait_path>::#trait_method(#wrapper),
            }
        }
        .to_tokens(tokens);
    }
}

pub enum IteratorExprStruct<'a, 'src> {
    Regular(IteratorExprStructRegular<'src>),
    Wrapper(IteratorExprStructWrapper<'a, 'src>),
}

impl ToTokens for IteratorExprStruct<'_, '_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            IteratorExprStruct::Regular(r) => r.to_tokens(tokens),
            IteratorExprStruct::Wrapper(w) => w.to_tokens(tokens),
        }
    }
}

pub struct IteratorImplStruct<'a, 'src> {
    head: IteratorTypeDefHead,
    iter: IteratorTypeDefStruct<'a, 'src>,
    item: MatchingItem<'src>,
}

impl IteratorImplStruct<'_, '_> {
    pub fn expr(&self) -> IteratorExprStruct<'_, '_> {
        match &self.iter {
            IteratorTypeDefStruct::Regular(r) => IteratorExprStruct::Regular(r.expr()),
            IteratorTypeDefStruct::Wrapper(w) => IteratorExprStruct::Wrapper(w.expr()),
        }
    }

    pub fn ty(&self) -> proc_macro2::TokenStream {
        let head = &self.head;
        let name = &head.iter_name;
        let (_, ty_generics, _) = head.generics.split_for_impl();
        quote! { #name #ty_generics }
    }
}

impl ToTokens for IteratorImplStruct<'_, '_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let iter = &self.iter;
        let item = &self.item;
        let impl_head = self.head.impl_head();
        quote! {
            #iter
            #impl_head {
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
                .trait_type_iter("Iter")
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
                .trait_type_iter("Iter")
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
