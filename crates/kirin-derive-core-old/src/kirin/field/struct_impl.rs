use quote::quote;

use crate::target;
use crate::prelude::*;

use super::context::FieldsIter;
use super::iter;

target! {
    /// Struct field iterator implementation
    pub struct StructImpl
}

impl<'src> Compile<'src, FieldsIter, StructImpl> for Struct<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> StructImpl {
        let iter: iter::StructImpl = self.compile(ctx);
        let trait_type_iter = &ctx.trait_type_iter;
        let trait_generics = ctx.generics();

        let unpacking = self.unpacking();
        let iter_expr: iter::StructExpr = self.compile(ctx);
        let iter_type: iter::FullType = self.compile(ctx);
        let trait_path: TraitPath = self.compile(ctx);

        let trait_impl = TraitImpl::default()
            .input(self.source())
            .trait_path(trait_path)
            .trait_generics(trait_generics.clone())
            .add_type(trait_type_iter, iter_type)
            .add_method(
                TraitItemFnImpl::new(&ctx.trait_method)
                    .with_self_lifetime(&ctx.trait_lifetime)
                    .with_mutable_self(ctx.mutable)
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
