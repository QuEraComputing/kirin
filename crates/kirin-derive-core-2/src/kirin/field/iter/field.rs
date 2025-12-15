/// implements iterator over fields of a struct or variant statement
/// e.g selecting `SSAValue` fields and iterating over them by
/// generating `std::iter::chain` of each field's iterator
use std::ops::Deref;

use crate::kirin::field::context::FieldsIter;
use crate::{
    data::*,
    kirin::{attrs::KirinFieldOptions, field::extra::FieldExtra},
};

use bon::Builder;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

impl<'src> Compile<'src, FieldsIter, DialectEnum<'src, FieldsIter>> for IteratorTypeDefHead {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectEnum<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        let mut head = Self::new(&node.input().ident, &ctx.iter_name, &ctx.trait_lifetime);
        if node.wraps || node.variants.iter().any(|v| v.wraps) {
            head.with_generics(&node.input().generics);
        }
        Ok(head)
    }
}

impl<'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>> for IteratorTypeDefHead {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        let mut head = Self::new(&node.input().ident, &ctx.iter_name, &ctx.trait_lifetime);
        if node.wraps {
            head.with_generics(&node.input().generics);
        }
        Ok(head)
    }
}

pub struct IteratorTypeDefHead {
    pub iter_name: syn::Ident,
    pub generics: syn::Generics,
}

impl IteratorTypeDefHead {
    /// generate impl head for the iterator type's Iterator impl
    /// e.g
    /// ```ignore
    /// impl<'trait_lifetime, ...> Iterator for <IterName><'trait_lifetime, ...>
    /// ```
    pub fn impl_head(&self) -> IteratorImplHead<'_> {
        IteratorImplHead(self)
    }

    /// return the type with generics applied (without bounds)
    /// e.g `<IterName><'trait_lifetime, ...>`
    /// assuming the generics have been set up correctly
    pub fn ty(&self) -> TokenStream {
        let iter_name = &self.iter_name;
        let (_, ty_generics, _) = &self.generics.split_for_impl();
        quote! {
            #iter_name #ty_generics
        }
    }
}

impl IteratorTypeDefHead {
    fn new<'a>(
        typename: &'a syn::Ident,
        iter_name: &'a syn::Ident,
        lifetime: &syn::Lifetime,
    ) -> Self {
        let mut generics = syn::Generics::default();
        generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(lifetime.clone())),
        );
        Self {
            iter_name: format_ident!("{}{}", typename, iter_name),
            generics,
        }
    }

    fn with_generics<'a>(&mut self, generics: &syn::Generics) -> &mut Self {
        for g in &generics.params {
            self.generics.params.push(g.clone());
        }
        self
    }
}

impl ToTokens for IteratorTypeDefHead {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let iter_name = &self.iter_name;
        let generics = &self.generics;
        quote! {
            #iter_name #generics
        }
        .to_tokens(tokens);
    }
}

pub struct IteratorImplHead<'a>(&'a IteratorTypeDefHead);

impl ToTokens for IteratorImplHead<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let iter_name = &self.0.iter_name;
        let (impl_generics, ty_generics, where_clause) = &self.0.generics.split_for_impl();
        quote! {
            #[automatically_derived]
            impl #impl_generics Iterator for #iter_name #ty_generics #where_clause
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::DeriveInput, FieldsIter>>
    for FieldIterator<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::DeriveInput, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(FieldIterator::builder()
            .mutable(ctx.mutable)
            .lifetime(&ctx.trait_lifetime)
            .matching_type(&ctx.matching_type)
            .fields(&node.fields)
            .build())
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for FieldIterator<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<FieldIterator<'src>> {
        Ok(FieldIterator::builder()
            .mutable(ctx.mutable)
            .lifetime(&ctx.trait_lifetime)
            .matching_type(&ctx.matching_type)
            .fields(&node.fields)
            .build())
    }
}

#[derive(Builder)]
pub struct MatchingItem<'src> {
    mutable: bool,
    lifetime: &'src syn::Lifetime,
    matching_type: &'src syn::Path,
}

impl<'src, T> Compile<'src, FieldsIter, T> for MatchingItem<'src> {
    fn compile(ctx: &'src FieldsIter, _node: &'src T) -> syn::Result<Self> {
        Ok(MatchingItem::builder()
            .mutable(ctx.mutable)
            .lifetime(&ctx.trait_lifetime)
            .matching_type(&ctx.matching_type)
            .build())
    }
}

impl ToTokens for MatchingItem<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let lifetime = &self.lifetime;
        let matching_type = &self.matching_type;
        if self.mutable {
            quote! { &#lifetime mut #matching_type }.to_tokens(tokens);
        } else {
            quote! { &#lifetime #matching_type }.to_tokens(tokens);
        }
    }
}

/// Iterator over fields of a struct or variant statement
/// generates the iterator object building code, e.g
/// ```ignore
/// std::iter::chain(
///    self.field1.iter(),
///   self.field2.iter(),
/// )
/// ```
#[derive(Builder)]
pub struct FieldIterator<'src> {
    mutable: bool,
    lifetime: &'src syn::Lifetime,
    matching_type: &'src syn::Path,
    fields: &'src Fields<'src, KirinFieldOptions, FieldExtra>,
    #[builder(default = MatchingItem::builder()
        .mutable(mutable)
        .lifetime(lifetime)
        .matching_type(matching_type)
        .build())]
    matching_item: MatchingItem<'src>,
}

impl<'src> FieldIterator<'src> {
    pub fn ty(&self) -> FieldIteratorType<'_, 'src> {
        FieldIteratorType(self)
    }
}

impl<'src> ToTokens for FieldIterator<'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let item = &self.matching_item;
        self.fields
            .iter()
            .filter_map(|f| match f.extra {
                FieldExtra::One => Some(quote! {
                    std::iter::once(#f)
                }),
                FieldExtra::Vec if self.mutable => Some(quote! {
                    #f.iter_mut()
                }),
                FieldExtra::Vec => Some(quote! {
                    #f.iter()
                }),
                FieldExtra::Other => None,
            })
            .fold(None, |acc: Option<TokenStream>, iter| {
                if let Some(acc) = acc {
                    Some(quote! { #acc.chain(#iter) })
                } else {
                    Some(iter)
                }
            })
            .unwrap_or(quote! { std::iter::empty::<#item>() })
            .to_tokens(tokens);
    }
}

pub struct FieldIteratorType<'iter, 'src>(&'iter FieldIterator<'src>);

impl<'src> Deref for FieldIteratorType<'_, 'src> {
    type Target = FieldIterator<'src>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> ToTokens for FieldIteratorType<'_, 'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let item = &self.matching_item;
        let lifetime = self.lifetime;
        let matching_type = self.matching_type;
        let new_tokens = self
            .fields
            .iter()
            .filter_map(|f| match f.extra {
                FieldExtra::One => Some(quote! { std::iter::Once<#item> }),
                FieldExtra::Vec if self.mutable => Some(quote! {
                    std::slice::IterMut<#lifetime, #matching_type>
                }),
                FieldExtra::Vec => Some(quote! {
                    std::slice::Iter<#lifetime, #matching_type>
                }),
                FieldExtra::Other => None,
            })
            .fold(None, |acc: Option<TokenStream>, ty| {
                if let Some(acc) = acc {
                    Some(quote! { std::iter::Chain<#acc, #ty>  })
                } else {
                    Some(ty)
                }
            })
            .unwrap_or(quote! { std::iter::Empty<#item> });
        tokens.extend(new_tokens);
    }
}
