use crate::data::{
    CrateRootPath, EnumAttribute, HasDefaultCratePath, HasTraitGenerics, SplitForImplTrait,
    enum_impl::variant_ref::VariantIter,
};

use super::variant_wrapper::WrapperVariant;

/// An enum that contains only wrapper instruction definitions.
pub struct WrapperEnum<'input, T: HasTraitGenerics> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: EnumAttribute,
    pub variants: Vec<WrapperVariant<'input, T>>,
}

#[bon::bon]
impl<'input, T: HasTraitGenerics> WrapperEnum<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<EnumAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| EnumAttribute::new(input));
        let syn::Data::Enum(data) = &input.data else {
            panic!("WrapperEnum can only be created from enum data");
        };
        let combined_generics = trait_info.combine_generics(&input.generics);

        let variants = data
            .variants
            .iter()
            .map(|variant| {
                WrapperVariant::builder()
                    .trait_info(trait_info)
                    .variant(variant)
                    .build()
            })
            .collect();

        Self {
            input,
            combined_generics,
            attrs,
            variants,
        }
    }

    pub fn iter(&self) -> VariantIter<'_, Self> {
        VariantIter {
            parent: self,
            current_index: 0,
            total_variants: self.variants.len(),
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }
}

impl<'input, T: HasTraitGenerics> std::fmt::Debug for WrapperEnum<'input, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WrapperEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for WrapperEnum<'input, T>
where
    T: HasTraitGenerics,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.trait_generics().split_for_impl();
        crate::data::SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause: where_clause.cloned(),
        }
    }
}

impl<'input, T> CrateRootPath<T> for WrapperEnum<'input, T>
where
    T: HasDefaultCratePath + HasTraitGenerics,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}
