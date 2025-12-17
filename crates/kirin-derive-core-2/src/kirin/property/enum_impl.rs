use super::context::{Property, SearchProperty};
use crate::{data::*, target};
use quote::{ToTokens, quote};

target! {
    pub struct EnumImpl
}

impl<'src, S: SearchProperty> Compile<'src, DialectEnum<'src, Property<S>>, EnumImpl>
    for Property<S>
{
    fn compile(&self, node: &DialectEnum<'src, Property<S>>) -> EnumImpl {
        let value_type = &self.value_type;
        let variant_ident = node.variant_idents();
        let unpacking = node.unpacking();
        let glob = S::search_globally_enum(node);
        let action = node
            .variants
            .iter()
            .map(|v| {
                if let Some(wrapper) = v.wrapper() {
                    let wrapper_type = &wrapper.src.ty;
                    let trait_path = &self.trait_path;
                    let trait_method = &self.trait_method;
                    quote! {
                        <#wrapper_type as #trait_path>::#trait_method(#wrapper)
                    }
                } else {
                    let value = S::search_in_statement(v);
                    let combined = S::combine(&glob, &value);
                    combined
                }
            })
            .collect::<Vec<_>>();

        let trait_path: TraitPath = self.compile(node);
        let trait_impl = TraitImpl::default()
            .input(node.source())
            .trait_path(trait_path)
            .add_method(
                TraitItemFnImpl::new(&self.trait_method)
                    .with_output(quote! {#value_type})
                    .with_token_body(quote! {
                        match self {
                            #(
                                Self::#variant_ident #unpacking => {
                                    #action
                                }
                            ),*
                        }
                    }),
            );
        trait_impl.to_token_stream().into()
    }
}
