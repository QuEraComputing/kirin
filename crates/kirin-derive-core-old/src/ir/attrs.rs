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

/// This attribute can provide an explicit crate path
/// as the root of the derive macro's paired runtime crate
/// usually via a global attribute like `#[kirin(crate = some::path)]`
pub trait WithUserCratePath {
    fn user_crate_path(&self) -> Option<&syn::Path>;
}

impl WithUserCratePath for () {
    fn user_crate_path(&self) -> Option<&syn::Path> {
        None
    }
}

impl<'src, L> WithUserCratePath for Input<'src, L>
where
    L: Layout,
    L::EnumAttr: WithUserCratePath,
    L::StructAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        match self {
            Input::Struct(s) => s.user_crate_path(),
            Input::Enum(e) => e.user_crate_path(),
        }
    }
}

impl<'src, L> WithUserCratePath for Struct<'src, L>
where
    L: Layout,
    L::StructAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.definition.0.attrs.user_crate_path()
    }
}

impl<'src, L> WithUserCratePath for Enum<'src, L>
where
    L: Layout,
    L::EnumAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.definition.attrs.user_crate_path()
    }
}

impl<'src, L> WithUserCratePath for Variant<'_, 'src, L>
where
    L: Layout,
    L::EnumAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.parent.attrs.user_crate_path()
    }
}

impl<'src, L> WithUserCratePath for Fields<'_, 'src, L>
where
    L: Layout,
    L::StructAttr: WithUserCratePath,
    L::EnumAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.parent.user_crate_path()
    }
}

impl<'src, L> WithUserCratePath for Field<'_, 'src, L>
where
    L: Layout,
    L::StructAttr: WithUserCratePath,
    L::EnumAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        self.parent.user_crate_path()
    }
}

impl<L> WithUserCratePath for DefinitionStructOrVariant<'_, L>
where
    L: Layout,
    L::StructAttr: WithUserCratePath,
    L::EnumAttr: WithUserCratePath,
{
    fn user_crate_path(&self) -> Option<&syn::Path> {
        match self {
            DefinitionStructOrVariant::Struct(s) => s.attrs.user_crate_path(),
            DefinitionStructOrVariant::Variant(e, _) => e.attrs.user_crate_path(),
        }
    }
}
