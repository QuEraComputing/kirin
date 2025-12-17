use quote::quote;

use super::expr::Expr;
use super::ty::InnerType;
use crate::data::*;
use crate::kirin::field::FieldsIter;
use crate::target;

pub type VariantTypeDef = Alt<WrapperVariantTypeDef, RegularVariantTypeDef>;

target! {
    /// Regular variant type definition
    pub struct RegularVariantTypeDef
}

impl<'src> Compile<'src, Statement<'src, syn::Variant, Self>, RegularVariantTypeDef>
    for FieldsIter
{
    fn compile(&self, node: &Statement<'src, syn::Variant, Self>) -> RegularVariantTypeDef {
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

impl<'src> Compile<'src, Statement<'src, syn::Variant, Self>, WrapperVariantTypeDef>
    for FieldsIter
{
    fn compile(&self, node: &Statement<'src, syn::Variant, Self>) -> WrapperVariantTypeDef {
        let variant_name = node.source_ident();
        let wrapped_type = node.wrapper_ty_tokens();
        let trait_path = &self.trait_path;
        let trait_type_iter = &self.trait_type_iter;
        WrapperVariantTypeDef(quote! {
            #variant_name (<#wrapped_type as #trait_path>::#trait_type_iter)
        })
    }
}

pub type TraitMatchArmBody = Alt<WrapperTraitMatchArmBody, RegularTraitMatchArmBody>;

target! {
    /// Regular trait match arm for variants
    pub struct RegularTraitMatchArmBody
}

impl<'src> Compile<'src, Statement<'src, syn::Variant, Self>, RegularTraitMatchArmBody> for FieldsIter {
    fn compile(&self, node: &Statement<'src, syn::Variant, Self>) -> RegularTraitMatchArmBody {
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

impl<'src> Compile<'src, Statement<'src, syn::Variant, Self>, WrapperTraitMatchArmBody> for FieldsIter {
    fn compile(&self, node: &Statement<'src, syn::Variant, Self>) -> WrapperTraitMatchArmBody {
        let variant_name = node.source_ident();
        let wrapper = node.wrapper_tokens();
        let wrapper_type = node.wrapper_ty_tokens();
        let trait_path = &self.trait_path;
        let trait_method = &self.trait_method;

        WrapperTraitMatchArmBody(quote! {
            #variant_name (<#wrapper_type as #trait_path>::#trait_method(#wrapper))
        })
    }
}
