use quote::quote;

use super::expr::Expr;
use super::ty::InnerType;
use crate::kirin::field::context::FieldsIter;
use crate::prelude::*;
use crate::target;

pub type VariantTypeDef = Alt<WrapperVariantTypeDef, RegularVariantTypeDef>;

target! {
    /// Regular variant type definition
    pub struct RegularVariantTypeDef
}

impl<'a, 'src> Compile<'src, FieldsIter, RegularVariantTypeDef> for Variant<'a, 'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> RegularVariantTypeDef {
        let variant_name = self.source_ident();
        let ty: InnerType = self.compile(ctx);
        quote! {
            #variant_name ( #ty )
        }
        .into()
    }
}

target! {
    /// Wrapper variant type definition
    pub struct WrapperVariantTypeDef
}

impl<'a, 'src> Compile<'src, FieldsIter, WrapperVariantTypeDef> for Variant<'a, 'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> WrapperVariantTypeDef {
        let variant_name = self.source_ident();
        let wrapped_type = self.wrapper_type_tokens();
        let trait_path = &ctx.trait_path;
        let (_, ty_generics, _) = &ctx.generics().split_for_impl();
        let trait_type_iter = &ctx.trait_type_iter;
        WrapperVariantTypeDef(quote! {
            #variant_name (<#wrapped_type as #trait_path #ty_generics>::#trait_type_iter)
        })
    }
}

pub type TraitMatchArmBody = Alt<WrapperTraitMatchArmBody, RegularTraitMatchArmBody>;

target! {
    /// Regular trait match arm for variants
    pub struct RegularTraitMatchArmBody
}

impl<'a, 'src> Compile<'src, FieldsIter, RegularTraitMatchArmBody> for Variant<'a, 'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> RegularTraitMatchArmBody {
        let variant_name = self.source_ident();
        let expr: Expr = self.compile(ctx);
        RegularTraitMatchArmBody(quote! {
            #variant_name ( #expr )
        })
    }
}

target! {
    /// Wrapper trait match arm for variants
    pub struct WrapperTraitMatchArmBody
}

impl<'a, 'src> Compile<'src, FieldsIter, WrapperTraitMatchArmBody> for Variant<'a, 'src, FieldsIter> {
    fn compile(&self, ctx: &FieldsIter) -> WrapperTraitMatchArmBody {
        let variant_name = self.source_ident();
        let wrapper = self.wrapper_tokens();
        let wrapper_type = self.wrapper_type_tokens();
        let trait_path = &ctx.trait_path;
        let trait_method = &ctx.trait_method;

        WrapperTraitMatchArmBody(quote! {
            #variant_name (<#wrapper_type as #trait_path>::#trait_method(#wrapper))
        })
    }
}
