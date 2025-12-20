use super::definition::*;

pub trait Attrs {
    type Output;
    /// Returns a reference to the attributes.
    #[must_use]
    fn attrs(&self) -> &Self::Output;
}

impl Attrs for syn::DeriveInput {
    type Output = Vec<syn::Attribute>;
    fn attrs(&self) -> &Self::Output {
        &self.attrs
    }
}

impl Attrs for syn::Variant {
    type Output = Vec<syn::Attribute>;
    fn attrs(&self) -> &Self::Output {
        &self.attrs
    }
}

impl Attrs for syn::Field {
    type Output = Vec<syn::Attribute>;
    fn attrs(&self) -> &Self::Output {
        &self.attrs
    }
}

impl<'src, L: Layout> Attrs for Struct<'src, L> {
    type Output = L::StructAttr;
    fn attrs(&self) -> &Self::Output {
        &self.definition.0.attrs
    }
}

impl<'src, L: Layout> Attrs for Enum<'src, L> {
    type Output = L::EnumAttr;
    fn attrs(&self) -> &Self::Output {
        &self.definition.attrs
    }
}

impl<'a, 'src, L: Layout> Attrs for Variant<'a, 'src, L> {
    type Output = L::VariantAttr;
    fn attrs(&self) -> &Self::Output {
        &self.parent.variants[self.index].0.attrs
    }
}

impl<'a, 'src, L: Layout> Attrs for Field<'a, 'src, L> {
    type Output = L::FieldAttr;
    fn attrs(&self) -> &Self::Output {
        &self.definition().attrs
    }
}
