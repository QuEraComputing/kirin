use quote::{ToTokens, quote};

use crate::data::gadgets::{TraitImpl, TraitItemFnImpl};
use crate::data::*;
use crate::kirin::attrs::KirinFieldOptions;
use crate::kirin::field::context::FieldsIter;
use crate::kirin::field::extra::FieldExtra;
use crate::kirin::field::iter::IteratorImplStruct;

impl<'a, 'src> Compile<'src, FieldsIter, DialectStruct<'src, FieldsIter>> for StructImpl<'a, 'src> {
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
            trait_type_iter: &ctx.trait_type_iter,
            fields: &node.statement.fields,
            iter: IteratorImplStruct::compile(ctx, node)?,
        })
    }
}

pub struct StructImpl<'a, 'src> {
    src: &'src syn::DeriveInput,
    mutable: bool,
    trait_name: &'src syn::Ident,
    trait_lifetime: &'src syn::Lifetime,
    trait_method: &'src syn::Ident,
    trait_type_iter: &'src syn::Ident,
    fields: &'src Fields<'src, KirinFieldOptions, FieldExtra>,
    iter: IteratorImplStruct<'a, 'src>,
}

impl ToTokens for StructImpl<'_, '_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let iter = &self.iter;
        let trait_type_iter = &self.trait_type_iter;

        let mut trait_generics = syn::Generics::default();
        trait_generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                self.trait_lifetime.clone(),
            )));

        let unpacking = self.fields.unpacking();
        let iter_expr = self.iter.expr();
        let trait_impl = TraitImpl::new(self.src, self.trait_name, &trait_generics)
            .add_type(trait_type_iter, self.iter.ty())
            .add_method(
                TraitItemFnImpl::new(self.trait_method)
                    .with_self_lifetime(self.trait_lifetime)
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
                .trait_type_iter("Iter")
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
