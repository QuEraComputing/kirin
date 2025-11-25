use crate::data::{FromVariantFields, StatementFields, VariantAttribute};

pub struct RegularVariant<'input, T: StatementFields<'input>> {
    pub variant: &'input syn::Variant,
    pub attrs: VariantAttribute,
    pub variant_name: &'input syn::Ident,
    pub fields: T::FieldsType,
}

#[bon::bon]
impl<'input, T: StatementFields<'input>> RegularVariant<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> syn::Result<Self> {
        let attrs = match attrs {
            Some(a) => a,
            None => VariantAttribute::new(variant)?,
        };
        let fields =
            T::FieldsType::from_variant_fields(&trait_info, &attrs, variant, &variant.fields)?;

        Ok(RegularVariant {
            variant,
            attrs,
            variant_name: &variant.ident,
            fields,
        })
    }
}

impl<'input, T> std::fmt::Debug for RegularVariant<'input, T>
where
    T: StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegularVariant")
            .field("variant_name", &self.variant_name)
            .field("fields", &self.fields)
            .finish()
    }
}
