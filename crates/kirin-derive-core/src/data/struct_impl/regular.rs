use crate::data::{
    CrateRootPath, FromStruct, FromStructFields, HasDefaultCratePath, HasTraitGenerics,
    SplitForImplTrait, StatementFields, StructAttribute,
};

pub struct RegularStruct<'input, T: HasTraitGenerics + StatementFields<'input>> {
    pub input: &'input syn::DeriveInput,
    pub combined_generics: syn::Generics,
    pub attrs: StructAttribute,
    pub struct_info: T::InfoType,
    pub fields: T::FieldsType,
}

#[bon::bon]
impl<'input, T> RegularStruct<'input, T>
where
    T: HasTraitGenerics + StatementFields<'input>,
{
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<StructAttribute>,
        input: &'input syn::DeriveInput,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| StructAttribute::new(input));
        let struct_info = T::InfoType::from_struct(&trait_info, &attrs, input);
        let fields = T::FieldsType::from_struct_fields(
            trait_info,
            &attrs,
            match &input.data {
                syn::Data::Struct(data) => data,
                _ => panic!("RegularStruct can only be created from struct data"),
            },
            match &input.data {
                syn::Data::Struct(data) => &data.fields,
                _ => panic!("RegularStruct can only be created from struct data"),
            },
        );
        let combined_generics = trait_info.combine_generics(&input.generics);

        RegularStruct {
            input,
            combined_generics,
            attrs,
            struct_info,
            fields,
        }
    }

    pub fn input(&self) -> &'input syn::DeriveInput {
        self.input
    }
}

impl<'a, 'input, T> SplitForImplTrait<'a, T> for RegularStruct<'input, T>
where
    T: HasTraitGenerics + StatementFields<'input>,
{
    fn split_for_impl(&'a self, trait_info: &'a T) -> crate::data::SplitForImpl<'a> {
        let (impl_generics, _, where_clause) = self.combined_generics.split_for_impl();
        let (_, input_ty_generics, _) = self.input.generics.split_for_impl();
        let (_, trait_ty_generics, _) = trait_info.trait_generics().split_for_impl();
        crate::data::SplitForImpl {
            impl_generics,
            trait_ty_generics,
            input_ty_generics,
            where_clause: where_clause.cloned(),
        }
    }
}

impl<'input, T> CrateRootPath<T> for RegularStruct<'input, T>
where
    T: HasDefaultCratePath + HasTraitGenerics + StatementFields<'input>,
{
    fn crate_root_path(&self, trait_info: &T) -> syn::Path {
        self.attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| trait_info.default_crate_path())
    }
}
