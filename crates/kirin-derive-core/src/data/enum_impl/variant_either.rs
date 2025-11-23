use crate::data::{HasTraitGenerics, StatementFields, VariantAttribute};

use super::variant_regular::RegularVariant;
use super::variant_wrapper::WrapperVariant;

pub enum EitherVariant<'input, T: HasTraitGenerics + StatementFields<'input>> {
    Wrapper(WrapperVariant<'input, T>),
    Regular(RegularVariant<'input, T>),
}

#[bon::bon]
impl<'input, T: HasTraitGenerics + StatementFields<'input>> EitherVariant<'input, T> {
    #[builder]
    pub fn new(
        trait_info: &T,
        attrs: Option<VariantAttribute>,
        variant: &'input syn::Variant,
    ) -> Self {
        let attrs = attrs.unwrap_or_else(|| VariantAttribute::new(variant));
        if attrs.is_wrapper() {
            EitherVariant::Wrapper(
                WrapperVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(Some(attrs))
                    .variant(variant)
                    .build(),
            )
        } else {
            EitherVariant::Regular(
                RegularVariant::builder()
                    .trait_info(trait_info)
                    .maybe_attrs(Some(attrs))
                    .variant(variant)
                    .build(),
            )
        }
    }
}

impl<'input, T> std::fmt::Debug for EitherVariant<'input, T>
where
    T: HasTraitGenerics + StatementFields<'input>,
    T::FieldsType: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EitherVariant::Wrapper(data) => f
                .debug_tuple("WrapperOrRegularVariant::Wrapper")
                .field(data)
                .finish(),
            EitherVariant::Regular(data) => f
                .debug_tuple("WrapperOrRegularVariant::Regular")
                .field(data)
                .finish(),
        }
    }
}
