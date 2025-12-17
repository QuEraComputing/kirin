use super::context::{Property, SearchProperty};
use crate::{data::*, target};
use quote::{ToTokens, quote};

target! {
    pub struct StructImpl
}

impl<'src, S: SearchProperty> Compile<'src, DialectStruct<'src, Self>, StructImpl> for Property<S> {
    fn compile(&self, node: &DialectStruct<'src, Self>) -> StructImpl {
        let trait_method = &self.trait_method;
        let trait_path: TraitPath = self.compile(node);
        let trait_fn_impl = if let Some(wrapper) = &node.wrapper() {
            let wrapper_type = &wrapper.src.ty;
            let unpacking = node.statement.fields.unpacking();
            TraitItemFnImpl::new(&self.trait_method)
                .with_output(&self.value_type)
                .with_token_body(quote! {
                    let Self #unpacking = self;
                    <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                })
        } else {
            let value = S::search_in_statement(&node.statement);
            TraitItemFnImpl::new(&self.trait_method)
                .with_output(&self.value_type)
                .with_token_body(quote! {
                    #value
                })
        };
        TraitImpl::default()
            .input(node.source())
            .trait_path(&self.trait_path)
            .add_method(trait_fn_impl)
            .to_token_stream()
            .into()
    }
}
