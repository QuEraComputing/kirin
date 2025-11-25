use crate::data::{
    CombineGenerics, CrateRootPath, EnumAttribute, FromEnum, HasDefaultCratePath, HasGenerics,
    SplitForImplTrait, StatementFields, enum_impl::variant_ref::VariantIter,
};

use super::variant_regular::RegularVariant;

/// An enum that contains only regular instruction definitions.
pub struct RegularEnum<'input, T: StatementFields<'input>> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: EnumAttribute,
    pub enum_info: T::InfoType,
    pub variants: Vec<RegularVariant<'input, T>>,
}

#[bon::bon]
impl<'input, T: CombineGenerics + StatementFields<'input>> RegularEnum<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<EnumAttribute>,
        input: &'input syn::DeriveInput,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => EnumAttribute::new(input)?,
        };
        let syn::Data::Enum(data) = &input.data else {
            return Err(syn::Error::new_spanned(
                input,
                "RegularEnum can only be created from enum data",
            ));
        };
        let enum_info = T::InfoType::from_enum(&trait_info, &attrs, input)?;
        let combined_generics = trait_info.combine_generics(&input.generics);

        let variants = data
            .variants
            .iter()
            .map(|variant| {
                RegularVariant::builder()
                    .trait_info(trait_info)
                    .variant(variant)
                    .build()
            })
            .collect::<syn::Result<Vec<_>>>()?;

        Ok(Self {
            input,
            combined_generics,
            attrs,
            enum_info,
            variants,
        })
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

impl<'input, T> std::fmt::Debug for RegularEnum<'input, T>
where
    T: StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegularEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for RegularEnum<'input, T>
where
    T: HasGenerics + StatementFields<'input>,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.generics().split_for_impl();
        crate::data::SplitForImpl {
            impl_generics,
            input_ty_generics,
            trait_ty_generics,
            where_clause: where_clause.cloned(),
        }
    }
}

impl<'input, T> CrateRootPath<T> for RegularEnum<'input, T>
where
    T: HasDefaultCratePath + StatementFields<'input>,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}
