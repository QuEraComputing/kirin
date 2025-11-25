use crate::data::{
    CombineGenerics, CrateRootPath, EnumAttribute, FromEnum, HasDefaultCratePath, HasGenerics, SplitForImplTrait, StatementFields, enum_impl::{variant_either::EitherVariant, variant_ref::VariantIter}
};

/// An enum that contains a mix of wrapper and regular instruction definitions.
pub struct EitherEnum<'input, T: StatementFields<'input>> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: EnumAttribute,
    pub enum_info: T::InfoType,
    pub variants: Vec<EitherVariant<'input, T>>,
}

#[bon::bon]
impl<'input, T: CombineGenerics + StatementFields<'input>> EitherEnum<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<EnumAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| EnumAttribute::new(input));
        let syn::Data::Enum(data) = &input.data else {
            panic!("EitherEnum can only be created from enum data");
        };
        let enum_info = T::InfoType::from_enum(&trait_info, &attrs, input);

        let combined_generics = trait_info.combine_generics(&input.generics);

        let variants = data
            .variants
            .iter()
            .map(|variant| {
                EitherVariant::builder()
                    .trait_info(trait_info)
                    .variant(variant)
                    .build()
            })
            .collect();

        Self {
            input,
            combined_generics,
            attrs,
            enum_info,
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

impl<'input, T> std::fmt::Debug for EitherEnum<'input, T>
where
    T: StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EitherEnum")
            .field("variants", &self.variants)
            .finish()
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for EitherEnum<'input, T>
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

impl<'input, T> CrateRootPath<T> for EitherEnum<'input, T>
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
