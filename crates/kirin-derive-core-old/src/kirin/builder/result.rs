use super::{context::Builder, name::StatementIdName};
use crate::kirin::extra::{FieldCollectionKind, FieldKind};
use crate::prelude::*;
use quote::{format_ident, quote};

target! {
    pub struct LetNameEqResultValue
}

impl<'src> Compile<'src, Builder, LetNameEqResultValue> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> LetNameEqResultValue {
        let result_names: ResultNames = self.compile(ctx);
        let results: Vec<TokenStream> = self
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .enumerate()
            .zip(result_names)
            .map(|((index, f), name)| {
                let ty = &f.source().ty;
                let statement_id: StatementIdName = self.compile(ctx);

                let Some(ssa_ty) = &f.attrs().ssa_ty else {
                    return syn::Error::new_spanned(
                        f.source(),
                        "expect #[kirin(type = ...)] attribute for 'ResultValue' field",
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

pub struct ResultNames(pub(super) std::vec::IntoIter<syn::Ident>);

impl<'src> Compile<'src, Builder, ResultNames> for Fields<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> ResultNames {
        let results = self
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .collect::<Vec<_>>();

        if results.len() == 1 {
            return ResultNames(
                results
                    .iter()
                    .map(|f| {
                        f.source().ident.clone().unwrap_or_else(|| {
                            format_ident!("result", span = f.source_ident().span())
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter(),
            );
        }

        let names: Vec<_> = results
            .iter()
            .enumerate()
            .map(|(index, f)| {
                f.source().ident.clone().unwrap_or_else(|| {
                    format_ident!("result_{}", index, span = f.source_ident().span())
                })
            })
            .collect();

        ResultNames(names.into_iter())
    }
}

impl Iterator for ResultNames {
    type Item = syn::Ident;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
