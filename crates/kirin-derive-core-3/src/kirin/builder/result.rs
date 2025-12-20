use super::context::{Builder, FieldKind};
use crate::{
    kirin::builder::{context::FieldCollectionKind, name::StatementIdName},
    prelude::*,
};
use quote::quote;

target! {
    pub struct LetNameEqResultValue
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, LetNameEqResultValue> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> LetNameEqResultValue {
        let results: Vec<TokenStream> = node
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .enumerate()
            .map(|(index, f)| {
                let name = f.source_ident();
                let ty = &f.source().ty;
                let statement_id: StatementIdName = self.compile(node);

                let Some(ssa_ty) = &f.attrs().ssa_ty else {
                    return syn::Error::new_spanned(
                        &f.source_ident(),
                        "expect #[kirin(type = ...)] attribute for ResultValue field",
                    )
                    .to_compile_error()
                    .into();
                };
                if matches!(f.extra().collection, FieldCollectionKind::Vec) {
                    return syn::Error::new_spanned(
                        &f.source_ident(),
                        "ResultValue field cannot be a Vec, consider implementing the builder manually",
                    )
                    .to_compile_error()
                    .into();
                } else if matches!(f.extra().collection, FieldCollectionKind::Option) {
                    return syn::Error::new_spanned(
                        &f.source_ident(),
                        "ResultValue field cannot be an Option, consider implementing the builder manually",
                    )
                    .to_compile_error()
                    .into();
                }

                quote! {
                    let #name: #ty = context
                        .ssa()
                        .kind(SSAKind::Result(#statement_id, #index))
                        .ty(Lang::TypeLattice::from(#ssa_ty))
                        .new()
                        .into();
                }
                .into()
            })
            .collect();

        quote! {
            #(#results)*
        }
        .into()
    }
}
