use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Variant;

use crate::{
    data::{
        Compile, DialectEnum, FieldMember, Statement,
        gadgets::{TraitImpl, TraitItemFnImpl},
    },
    kirin::{
        attrs::KirinFieldOptions,
        property::context::{Property, SearchProperty},
    },
};

pub struct EnumImpl<'a, 'src> {
    src: &'src syn::DeriveInput,
    trait_path: &'src syn::Path,
    trait_method: &'src syn::Ident,
    value_type: TokenStream,
    variants: Vec<VariantImpl<'a, 'src>>,
}

impl<'a, 'src, S: SearchProperty> Compile<'src, Property<S>, DialectEnum<'src, Property<S>>>
    for EnumImpl<'a, 'src>
{
    fn compile(
        ctx: &'src Property<S>,
        node: &'src DialectEnum<'src, Property<S>>,
    ) -> syn::Result<Self> {
        let glob = S::search_globally_enum(node);
        let value_type = ctx.value_type.to_token_stream();
        let mut variants = node
            .variants
            .iter()
            .map(|v| VariantImpl::compile(ctx, v))
            .collect::<syn::Result<Vec<_>>>()?;

        for variant in variants.iter_mut() {
            variant.value = S::combine(&glob, &variant.value);
        }

        Ok(EnumImpl {
            src: node.input(),
            trait_path: &ctx.trait_path,
            trait_method: &ctx.trait_method,
            value_type,
            variants,
        })
    }
}

impl<'a, 'src> ToTokens for EnumImpl<'a, 'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variants = &self.variants;
        let trait_fn_impl = TraitItemFnImpl::new(self.trait_method)
            .with_output(&self.value_type)
            .with_token_body(quote! {
                match self {
                    #(#variants),*
                }
            });

        TraitImpl::new()
            .input(self.src)
            .trait_path(self.trait_path)
            .add_method(trait_fn_impl)
            .to_tokens(tokens);
    }
}

pub struct VariantImpl<'a, 'src> {
    src: &'src syn::Variant,
    trait_path: &'src syn::Path,
    trait_method: &'src syn::Ident,
    wrapper: Option<FieldMember<'a, 'src, KirinFieldOptions, ()>>,
    value: TokenStream,
}

impl<'a, 'src, S: SearchProperty>
    Compile<'src, Property<S>, Statement<'src, syn::Variant, Property<S>>>
    for VariantImpl<'a, 'src>
{
    fn compile(
        ctx: &'src Property<S>,
        node: &'src Statement<'src, syn::Variant, Property<S>>,
    ) -> syn::Result<Self> {
        let value = S::search_in_statement(node);
        Ok(VariantImpl {
            src: node.src,
            trait_path: &ctx.trait_path,
            trait_method: &ctx.trait_method,
            wrapper: node.fields.wrapper(),
            value,
        })
    }
}

impl<'a, 'src> ToTokens for VariantImpl<'a, 'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variant_name = &self.src.ident;

        if let Some(wrapper) = &self.wrapper {
            let wrapper_type = &wrapper.src.ty;
            let trait_path = self.trait_path;
            let trait_method = self.trait_method;
            quote! {
                Self::#variant_name { .. } => {
                    <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                }
            }
        } else {
            let value = &self.value;
            quote! {
                Self::#variant_name { .. } => {
                    #value
                }
            }
        }
        .to_tokens(tokens);
    }
}
