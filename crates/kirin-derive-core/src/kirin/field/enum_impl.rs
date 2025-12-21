use quote::quote;

use crate::prelude::*;
use crate::kirin::field::iter;
use crate::kirin::field::iter::TraitMatchArmBody;
use crate::target;

use super::context::FieldsIter;

target! {
    /// Enum field iterator implementation
    pub struct EnumImpl
}

impl<'src> Compile<'src, Enum<'src, FieldsIter>, EnumImpl> for FieldsIter {
    fn compile(&self, node: &Enum<'src, FieldsIter>) -> EnumImpl {
        let trait_type_iter = &self.trait_type_iter;
        let trait_generics = self.generics();

        let iter: iter::EnumImpl = self.compile(node);
        let iter_name: iter::Name = self.compile(node);
        let iter_type: iter::FullType = self.compile(node);
        let variant_ident = node.variant_names();
        let unpacking = node.unpacking();
        let action: Action<TraitMatchArmBody> = self.compile(node);
        let trait_path: TraitPath = self.compile(node);

        let trait_impl = TraitImpl::default()
            .input(node.source())
            .trait_path(trait_path)
            .trait_generics(trait_generics.clone())
            .add_type(trait_type_iter, iter_type)
            .add_method(
                TraitItemFnImpl::new(&self.trait_method)
                    .with_self_lifetime(&self.trait_lifetime)
                    .with_mutable_self(self.mutable)
                    .with_output(quote! {Self::#trait_type_iter})
                    .with_token_body(quote! {
                        match self {
                            #(
                                Self::#variant_ident #unpacking => {
                                    #iter_name::#action
                                }
                            ),*
                        }
                    }),
            );

        quote! {
            #trait_impl
            #iter
        }
        .into()
    }
}
