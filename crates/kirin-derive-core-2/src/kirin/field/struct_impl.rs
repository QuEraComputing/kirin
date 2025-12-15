use quote::{ToTokens, quote};

use crate::data::gadgets::{TraitImpl, TraitItemFnImpl};
use crate::data::*;
use crate::kirin::field::context::FieldsIter;
use crate::kirin::field::iter::IteratorImplStruct;

impl<'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>> for StructImpl<'src> {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectStruct<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(StructImpl {
            src: node.input(),
            mutable: ctx.mutable,
            trait_name: &ctx.trait_name,
            trait_lifetime: &ctx.trait_lifetime,
            trait_method: &ctx.trait_method,
            iter: IteratorImplStruct::compile(ctx, node)?,
        })
    }
}

pub struct StructImpl<'src> {
    src: &'src syn::DeriveInput,
    mutable: bool,
    trait_name: &'src syn::Ident,
    trait_lifetime: &'src syn::Lifetime,
    trait_method: &'src syn::Ident,
    iter: IteratorImplStruct<'src>,
}

impl ToTokens for StructImpl<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let iter = &self.iter;

        let mut trait_generics = syn::Generics::default();
        trait_generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                self.trait_lifetime.clone(),
            )));

        let trait_impl = TraitImpl::new(self.src, self.trait_name, &trait_generics)
            .add_type(quote! {Type}, self.iter.expr().ty())
            .add_method(
                TraitItemFnImpl::new(self.trait_method)
                    .with_self_lifetime(self.trait_lifetime)
                    .with_mutable_self(self.mutable)
                    .with_output(quote! {Self::Iter})
                    .with_token_body(self.iter.expr()),
            );

        quote! {
            #trait_impl
            #iter
        }
        .to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kirin::field::context::FieldsIter;

    #[test]
    fn test_struct_iterator_impl_tokens() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = "MyLattice")]
            struct MyStruct<T: Bound> {
                values: Vec<SSAValue>,
                count: SSAValue,
                marker: std::marker::PhantomData<T>,
            }
        };

        for mutable in [false, true] {
            let ctx = FieldsIter::builder()
                .mutable(mutable)
                .trait_lifetime("'a")
                .matching_type("SSAValue")
                .default_crate_path("kirin::ir")
                .trait_path("HasArguments")
                .trait_method("arguments")
                .build();
            let data = DialectStruct::from_context(&ctx, &input).unwrap();
            let t = syn::parse_file(
                &StructImpl::compile(&ctx, &data)
                    .unwrap()
                    .into_token_stream()
                    .to_string(),
            )
            .unwrap();
            insta::assert_snapshot!(prettyplease::unparse(&t));
        }
    }
}
