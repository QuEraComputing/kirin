use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::context::Property;
use crate::data::*;
use crate::kirin::attrs::KirinFieldOptions;
use crate::{
    data::gadgets::{TraitImpl, TraitItemFnImpl},
    kirin::property::context::SearchProperty,
};

pub struct StructImpl<'a, 'src> {
    src: &'src syn::DeriveInput,
    trait_path: &'src syn::Path,
    trait_method: &'src syn::Ident,
    wrapper: Option<FieldMember<'a, 'src, KirinFieldOptions, ()>>,
    value: TokenStream,
    value_type: TokenStream,
}

impl<'a, 'src, S: SearchProperty> Compile<'src, Property<S>, DialectStruct<'src, Property<S>>>
    for StructImpl<'a, 'src>
{
    fn compile(
        ctx: &'src Property<S>,
        node: &'src DialectStruct<'src, Property<S>>,
    ) -> syn::Result<Self> {
        let value = S::search_in_statement(&node.statement);
        let value_type = ctx.value_type.to_token_stream();

        Ok(StructImpl {
            src: node.input(),
            trait_path: &ctx.trait_path,
            trait_method: &ctx.trait_method,
            wrapper: node.statement.fields.wrapper(),
            value,
            value_type,
        })
    }
}

impl<'a, 'src> ToTokens for StructImpl<'a, 'src> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let trait_path = self.trait_path;
        let trait_method = self.trait_method;
        let trait_fn_impl = if let Some(wrapper) = &self.wrapper {
            let wrapper_type = &wrapper.src.ty;
            TraitItemFnImpl::new(self.trait_method)
                .with_output(&self.value_type)
                .with_token_body(quote! {
                    <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                })
        } else {
            let value = &self.value;
            TraitItemFnImpl::new(self.trait_method)
                    .with_output(&self.value_type)
                    .with_token_body(quote! {
                        #value
                    })
        };

        TraitImpl::new()
            .input(self.src)
            .trait_path(self.trait_path)
            .add_method(trait_fn_impl)
            .to_tokens(tokens);
    }
}
