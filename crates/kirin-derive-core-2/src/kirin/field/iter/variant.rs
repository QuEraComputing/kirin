use super::field::FieldIterator;
use crate::kirin::field::context::FieldsIter;
use crate::kirin::field::extra::FieldExtra;
use crate::{data::*, kirin::attrs::KirinFieldOptions};

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for IteratorTypeDefVariant<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        if node.wraps {
            Ok(IteratorTypeDefVariant::Wrapper(
                IteratorTypeDefVariantWrapper::compile(ctx, node)?,
            ))
        } else {
            Ok(IteratorTypeDefVariant::Regular(
                IteratorTypeDefVariantRegular::compile(ctx, node)?,
            ))
        }
    }
}

pub enum IteratorTypeDefVariant<'a> {
    Regular(IteratorTypeDefVariantRegular<'a>),
    Wrapper(IteratorTypeDefVariantWrapper<'a>),
}

impl<'a> ToTokens for IteratorTypeDefVariant<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            IteratorTypeDefVariant::Regular(v) => v.to_tokens(tokens),
            IteratorTypeDefVariant::Wrapper(v) => v.to_tokens(tokens),
        }
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for IteratorTypeDefVariantRegular<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        let field_iterator = FieldIterator::compile(ctx, node)?;
        Ok(IteratorTypeDefVariantRegular {
            variant_name: &node.src.ident,
            field_iterator,
        })
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for IteratorTypeDefVariantWrapper<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        let f = node.fields.iter().find(|f| f.wraps).ok_or_else(|| {
            syn::Error::new_spanned(
                node.src,
                "Cannot create IteratorVariantWrapper: no field marked as wraps",
            )
        })?;
        Ok(IteratorTypeDefVariantWrapper {
            variant_name: &node.src.ident,
            wrapped_type: &f.src.ty,
            trait_path: &ctx.trait_path,
        })
    }
}

pub struct IteratorTypeDefVariantRegular<'a> {
    variant_name: &'a syn::Ident,
    field_iterator: FieldIterator<'a>,
}

impl<'a> ToTokens for IteratorTypeDefVariantRegular<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variant_name = self.variant_name;
        let ty = &self.field_iterator.ty();
        quote! {
            #variant_name ( #ty )
        }
        .to_tokens(tokens);
    }
}

pub struct IteratorTypeDefVariantWrapper<'a> {
    variant_name: &'a syn::Ident,
    wrapped_type: &'a syn::Type,
    trait_path: &'a syn::Path,
}

impl<'a> ToTokens for IteratorTypeDefVariantWrapper<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let variant_name = self.variant_name;
        let wrapped_type = self.wrapped_type;
        let trait_path = self.trait_path;
        quote! {
            #variant_name (<#wrapped_type as #trait_path>::Iter)
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for TraitMatchArmVariant<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        if node.wraps {
            Ok(TraitMatchArmVariant::Wrapper(
                TraitMatchArmVariantWrapper::compile(ctx, node)?,
            ))
        } else {
            Ok(TraitMatchArmVariant::Regular(
                TraitMatchArmVariantRegular::compile(ctx, node)?,
            ))
        }
    }
}

pub enum TraitMatchArmVariant<'src> {
    Regular(TraitMatchArmVariantRegular<'src>),
    Wrapper(TraitMatchArmVariantWrapper<'src>),
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for TraitMatchArmVariantRegular<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        let field_iterator = FieldIterator::compile(ctx, node)?;
        Ok(TraitMatchArmVariantRegular {
            iter_name: &ctx.iter_name,
            src: &node.src,
            fields: &node.fields,
            field_iterator,
        })
    }
}

impl<'a, 'src> ToTokens for TraitMatchArmVariant<'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TraitMatchArmVariant::Regular(v) => v.to_tokens(tokens),
            TraitMatchArmVariant::Wrapper(v) => v.to_tokens(tokens),
        }
    }
}

pub struct TraitMatchArmVariantRegular<'src> {
    iter_name: &'src syn::Ident,
    src: &'src syn::Variant,
    fields: &'src Fields<'src, KirinFieldOptions, FieldExtra>,
    field_iterator: FieldIterator<'src>,
}

impl<'src> ToTokens for TraitMatchArmVariantRegular<'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let iter: &syn::Ident = &self.iter_name;
        let variant_name = &self.src.ident;
        let unpacking = self.fields.unpacking();
        let iterator = &self.field_iterator;

        quote! {
            Self::#variant_name #unpacking => {
                #iter::#variant_name ( #iterator )
            }
        }
        .to_tokens(tokens);
    }
}

impl<'src> Compile<'src, FieldsIter, Statement<'src, syn::Variant, FieldsIter>>
    for TraitMatchArmVariantWrapper<'src>
{
    fn compile(
        ctx: &'src FieldsIter,
        node: &'src Statement<'src, syn::Variant, FieldsIter>,
    ) -> syn::Result<Self> {
        Ok(TraitMatchArmVariantWrapper {
            iter_name: &ctx.iter_name,
            src: &node.src,
            fields: &node.fields,
            trait_path: &ctx.trait_path,
            trait_method: &ctx.trait_method,
        })
    }
}

pub struct TraitMatchArmVariantWrapper<'src> {
    iter_name: &'src syn::Ident,
    src: &'src syn::Variant,
    fields: &'src Fields<'src, KirinFieldOptions, FieldExtra>,
    trait_path: &'src syn::Path,
    trait_method: &'src syn::Ident,
}

impl<'src> ToTokens for TraitMatchArmVariantWrapper<'src> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let iter: &syn::Ident = &self.iter_name;
        let variant_name = &self.src.ident;
        let wrapper = self.fields.wrapper().unwrap();
        let wrapper_type = &wrapper.src.ty;
        let unpacking = self.fields.unpacking();
        let trait_method = self.trait_method;
        let trait_path = self.trait_path;
        quote! {
            Self::#variant_name #unpacking => {
                #iter::#variant_name (<#wrapper_type as #trait_path>::#trait_method(#wrapper))
            }
        }
        .to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kirin::field::context::FieldsIter;

    #[test]
    fn test_iterator_variant_regular_tokens() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum MyEnum<T: Bound> {
                A(T, SSAValue),
                B(SSAValue, SSAValue, f64),
                C(Vec<SSAValue>),
            }
        };
        let syn::Data::Enum(data) = &input.data else {
            panic!("Expected enum data");
        };
        let ctx = FieldsIter::builder()
            .mutable(true)
            .trait_lifetime("'a")
            .matching_type("SSAValue")
            .default_crate_path("kirin::ir")
            .trait_path("HasParams")
            .trait_method("params")
            .build();

        for node in &data.variants {
            let data = Statement::from_context(&ctx, node).unwrap();
            let variant = IteratorTypeDefVariantRegular::compile(&ctx, &data).unwrap();
            insta::assert_snapshot!(variant.to_token_stream().to_string());
        }
    }
}
