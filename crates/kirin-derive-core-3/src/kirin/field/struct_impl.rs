use quote::quote;

use crate::target;
use crate::prelude::*;

use super::context::FieldsIter;
use super::iter;

target! {
    /// Struct field iterator implementation
    pub struct StructImpl
}

impl<'src> Compile<'src, Struct<'src, FieldsIter>, StructImpl> for FieldsIter {
    fn compile(&self, node: &Struct<'src, FieldsIter>) -> StructImpl {
        let iter: iter::StructImpl = self.compile(node);
        let trait_type_iter = &self.trait_type_iter;
        let trait_generics = self.generics();

        let unpacking = node.unpacking();
        let iter_expr: iter::StructExpr = self.compile(node);
        let iter_type: iter::FullType = self.compile(node);
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
                        let Self #unpacking = self;
                        #iter_expr
                    }),
            );
        
        quote! {
            #trait_impl
            #iter
        }.into()
    }
}
