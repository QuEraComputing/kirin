use quote::{ToTokens, format_ident, quote};

use crate::{
    data::{
        Compile, DialectEnum,
        gadgets::{TraitImpl, TraitItemFnImpl},
    },
    kirin::field::{
        context::FieldsIter,
        iter::{IteratorImplEnum, TraitMatchArmVariant},
    },
};

impl<'src> Compile<'src, FieldsIter, DialectEnum<'src, FieldsIter>> for EnumImpl<'src> {
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src DialectEnum<'src, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(EnumImpl {
            src: node.input(),
            mutable: ctx.mutable,
            trait_name: &ctx.trait_name,
            trait_lifetime: &ctx.trait_lifetime,
            trait_method: &ctx.trait_method,
            variants: node
                .variants
                .iter()
                .map(|v| TraitMatchArmVariant::compile(ctx, v))
                .collect::<syn::Result<Vec<_>>>()?,
            iter: IteratorImplEnum::compile(ctx, node)?,
        })
    }
}

pub struct EnumImpl<'src> {
    src: &'src syn::DeriveInput,
    mutable: bool,
    trait_name: &'src syn::Ident,
    trait_lifetime: &'src syn::Lifetime,
    trait_method: &'src syn::Ident,
    variants: Vec<TraitMatchArmVariant<'src>>,
    iter: IteratorImplEnum<'src>,
}

impl ToTokens for EnumImpl<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let iter = &self.iter;

        let mut trait_generics = syn::Generics::default();
        trait_generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                self.trait_lifetime.clone(),
            )));
        let arms = &self.variants;
        let trait_impl = TraitImpl::new(self.src, self.trait_name, &trait_generics)
            .add_type(format_ident!("Iter"), self.iter.ty())
            .add_method(
                TraitItemFnImpl::new(self.trait_method)
                    .with_self_lifetime(self.trait_lifetime)
                    .with_mutable_self(self.mutable)
                    .with_output(quote! {Self::Iter})
                    .with_token_body(quote! {
                        match self {
                            #(#arms),*
                        }
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
    use crate::data::*;
    use crate::kirin::field::context::FieldsIter;
    #[test]
    fn test_enum_impl() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = MyTypeLattice)]
            enum MyEnum {
                A(i32),
                B(String),
                C,
            }
        };

        let ctx = FieldsIter::builder()
            .mutable(false)
            .trait_lifetime("'a")
            .matching_type("SSAValue")
            .default_crate_path("kirin::ir")
            .trait_path("HasArguments")
            .trait_method("arguments")
            .build();

        let data = DialectEnum::from_context(&ctx, &input).unwrap();
        let content = EnumImpl::compile(&ctx, &data)
            .unwrap()
            .into_token_stream()
            .to_string();
        println!("{}", &content);
        let t = syn::parse_file(&content).unwrap();
        insta::assert_snapshot!(prettyplease::unparse(&t));
    }
}
