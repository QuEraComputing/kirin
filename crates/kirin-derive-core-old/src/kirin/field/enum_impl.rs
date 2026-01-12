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

impl<'src> Compile<'src, FieldsIter, EnumImpl> for Enum<'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> EnumImpl {
        let trait_type_iter = &ctx.trait_type_iter;
        let trait_generics = ctx.generics();

        let iter: iter::EnumImpl = self.compile(ctx);
        let iter_name: iter::Name = self.compile(ctx);
        let iter_type: iter::FullType = self.compile(ctx);
        let variant_ident = self.variant_names();
        let unpacking = self.unpacking();
        let action: Action<TraitMatchArmBody> = self.compile(ctx);
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
