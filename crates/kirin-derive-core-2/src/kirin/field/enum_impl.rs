use quote::quote;

use crate::data::*;
use crate::kirin::field::iter;
use crate::kirin::field::iter::TraitMatchArmBody;
use crate::target;

use super::FieldsIter;

target! {
    /// Enum field iterator implementation
    pub struct EnumImpl
}

impl<'src> Compile<'src, DialectEnum<'src, FieldsIter>, EnumImpl> for FieldsIter {
    fn compile(&self, node: &DialectEnum<'src, FieldsIter>) -> EnumImpl {
        let trait_type_iter = &self.trait_type_iter;
        let trait_generics = self.generics();

        let iter: iter::EnumImpl = self.compile(node);
        let iter_name: iter::Name = self.compile(node);
        let iter_type: iter::FullType = self.compile(node);
        let variant_ident = node.variant_idents();
        let unpacking = node.unpacking();
        // let action: Vec<TraitMatchArmBody> = node.variants.iter().map(|v| self.compile(v)).collect();
        let action: Vec<TraitMatchArmBody> = node.match_action(self);
        let trait_path: TraitPath = self.compile(node);

        let trait_impl = TraitImpl::default()
            .input(node.source())
            .trait_path(trait_path)
            .trait_generics(trait_generics)
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
