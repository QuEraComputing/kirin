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

impl<'a, 'src> Compile<'src, Variant<'a, 'src, Self>, RegularVariantTypeDef> for FieldsIter {
    fn compile(&self, node: &Variant<'a, 'src, Self>) -> RegularVariantTypeDef {
        let variant_name = node.source_ident();
        let ty: InnerType = self.compile(node);
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

impl<'a, 'src> Compile<'src, Variant<'a, 'src, Self>, WrapperVariantTypeDef> for FieldsIter {
    fn compile(&self, node: &Variant<'a, 'src, Self>) -> WrapperVariantTypeDef {
        let variant_name = node.source_ident();
        let wrapped_type = node.wrapper_type_tokens();
        let trait_path = &self.trait_path;
        let (_, ty_generics, _) = &self.generics().split_for_impl();
        let trait_type_iter = &self.trait_type_iter;
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

impl<'a, 'src> Compile<'src, Variant<'a, 'src, Self>, RegularTraitMatchArmBody> for FieldsIter {
    fn compile(&self, node: &Variant<'a, 'src, Self>) -> RegularTraitMatchArmBody {
        let variant_name = node.source_ident();
        let expr: Expr = self.compile(node);
        RegularTraitMatchArmBody(quote! {
            #variant_name ( #expr )
        })
    }
}

target! {
    /// Wrapper trait match arm for variants
    pub struct WrapperTraitMatchArmBody
}

impl<'a, 'src> Compile<'src, Variant<'a, 'src, Self>, WrapperTraitMatchArmBody> for FieldsIter {
    fn compile(&self, node: &Variant<'a, 'src, Self>) -> WrapperTraitMatchArmBody {
        let variant_name = node.source_ident();
        let wrapper = node.wrapper_tokens();
        let wrapper_type = node.wrapper_type_tokens();
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;

        WrapperTraitMatchArmBody(quote! {
            #variant_name (<#wrapper_type as #trait_path>::#trait_method(#wrapper))
        })
    }
}
