use std::ops::{Deref, DerefMut};

#[cfg(feature = "debug")]
use std::fmt::Debug;

#[cfg(feature = "debug")]
pub trait Layout {
    type StructAttr: Debug + darling::FromDeriveInput;
    type EnumAttr: Debug + darling::FromDeriveInput;
    type VariantAttr: Debug + darling::FromVariant;
    type FieldAttr: Debug + darling::FromField;
    type StatementExtra: Debug;
    type FieldExtra: Debug;
}

#[cfg(not(feature = "debug"))]
pub trait Layout {
    type StructAttr: darling::FromDeriveInput;
    type EnumAttr: darling::FromDeriveInput;
    type VariantAttr: darling::FromVariant;
    type FieldAttr: darling::FromField;
    type StatementExtra;
    type FieldExtra;
}

pub trait EmptyLayout {}

impl<T: EmptyLayout> Layout for T {
    type StructAttr = ();
    type EnumAttr = ();
    type VariantAttr = ();
    type FieldAttr = ();
    type StatementExtra = ();
    type FieldExtra = ();
}

#[cfg(feature = "debug")]
#[cfg_attr(feature = "debug", derive(Debug))]
pub struct EmptyLayoutImpl;

#[cfg(feature = "debug")]
impl EmptyLayout for EmptyLayoutImpl {}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct DefinitionStruct<L: Layout>(pub(super) DefinitionStatement<L::StructAttr, L>);

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct DefinitionEnum<L: Layout> {
    pub(super) attrs: L::EnumAttr,
    pub(super) marked_wraps: bool,
    pub(super) variants: Vec<DefinitionVariant<L>>,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct DefinitionVariant<L: Layout>(pub(super) DefinitionStatement<L::VariantAttr, L>);

impl<L: Layout> Deref for DefinitionStruct<L> {
    type Target = DefinitionStatement<L::StructAttr, L>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<L: Layout> DerefMut for DefinitionStruct<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<L: Layout> Deref for DefinitionVariant<L> {
    type Target = DefinitionStatement<L::VariantAttr, L>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<L: Layout> DerefMut for DefinitionVariant<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct DefinitionStatement<Attr, L: Layout> {
    pub(super) attrs: Attr,
    pub(super) wraps: bool,
    pub(super) fields: Vec<DefinitionField<L>>,
    pub(super) extra: L::StatementExtra,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub enum DefinitionStructOrVariant<'a, L: Layout> {
    Struct(&'a DefinitionStruct<L>),
    Variant(&'a DefinitionVariant<L>),
}

impl<L: Layout> Clone for DefinitionStructOrVariant<'_, L> {
    fn clone(&self) -> Self {
        match self {
            DefinitionStructOrVariant::Struct(s) => DefinitionStructOrVariant::Struct(s),
            DefinitionStructOrVariant::Variant(v) => DefinitionStructOrVariant::Variant(v),
        }
    }
}

impl<'a, L: Layout> DefinitionStructOrVariant<'a, L> {
    pub fn is_wrapper(&self) -> bool {
        match self {
            DefinitionStructOrVariant::Struct(s) => s.wraps,
            DefinitionStructOrVariant::Variant(v) => v.wraps,
        }
    }

    pub fn fields(&self) -> &Vec<DefinitionField<L>> {
        match self {
            DefinitionStructOrVariant::Struct(s) => &s.fields,
            DefinitionStructOrVariant::Variant(v) => &v.fields,
        }
    }

    pub fn extra(&self) -> &L::StatementExtra {
        match self {
            DefinitionStructOrVariant::Struct(s) => &s.extra,
            DefinitionStructOrVariant::Variant(v) => &v.extra,
        }
    }
}

impl<'a, L: Layout> From<&'a DefinitionStruct<L>> for DefinitionStructOrVariant<'a, L> {
    fn from(s: &'a DefinitionStruct<L>) -> Self {
        DefinitionStructOrVariant::Struct(s)
    }
}

impl<'a, L: Layout> From<&'a DefinitionVariant<L>> for DefinitionStructOrVariant<'a, L> {
    fn from(v: &'a DefinitionVariant<L>) -> Self {
        DefinitionStructOrVariant::Variant(v)
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct DefinitionField<L: Layout> {
    pub(super) wraps: bool,
    pub(super) attrs: L::FieldAttr,
    pub(super) extra: L::FieldExtra,
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub enum Input<'src, L: Layout = EmptyLayoutImpl> {
    Struct(Struct<'src, L>),
    Enum(Enum<'src, L>),
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Struct<'src, L: Layout> {
    pub(super) input: &'src syn::DeriveInput,
    pub(super) src: &'src syn::DataStruct,
    pub(super) definition: DefinitionStruct<L>,
}

impl<'src, L: Layout> Struct<'src, L> {
    pub(super) fn definition(&self) -> &DefinitionStruct<L> {
        &self.definition
    }

    pub fn is_wrapper(&self) -> bool {
        self.definition().0.wraps
    }

    pub fn extra(&self) -> &L::StatementExtra {
        &self.definition().0.extra
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Enum<'src, L: Layout> {
    pub(super) input: &'src syn::DeriveInput,
    pub(super) src: &'src syn::DataEnum,
    pub(super) definition: DefinitionEnum<L>,
}

impl<'src, L: Layout> Enum<'src, L> {
    pub(super) fn definition(&self) -> &DefinitionEnum<L> {
        &self.definition
    }

    pub fn marked_wraps(&self) -> bool {
        self.definition().marked_wraps
    }
}

impl<'src, L: Layout> Enum<'src, L> {
    pub fn for_each_variant<F>(&'src self, mut f: F)
    where
        F: FnMut(Variant<'_, 'src, L>),
    {
        for v in self.variants() {
            f(v);
        }
    }

    #[must_use]
    pub fn variants<'a>(&'a self) -> impl Iterator<Item = Variant<'a, 'src, L>> {
        self.src
            .variants
            .iter()
            .enumerate()
            .map(move |(index, src)| Variant {
                input: self.input,
                src,
                parent: &self.definition,
                index,
            })
    }

    #[must_use]
    pub fn variant_names(&self) -> Vec<&syn::Ident> {
        self.variants()
            .map(|v| &v.src.ident)
            .collect::<Vec<&syn::Ident>>()
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Variant<'a, 'src, L: Layout> {
    pub(super) input: &'src syn::DeriveInput,
    pub(super) src: &'src syn::Variant,
    pub(super) parent: &'a DefinitionEnum<L>,
    pub(super) index: usize,
}

impl<L: Layout> Variant<'_, '_, L> {
    pub(super) fn definition(&self) -> &DefinitionVariant<L> {
        &self.parent.variants[self.index]
    }

    pub fn extra(&self) -> &L::StatementExtra {
        &self.definition().0.extra
    }

    pub fn is_wrapper(&self) -> bool {
        // assume we guarantee that if the fields has a wrapper field, the variant is a wrapper
        // despite of it is marked as #[wraps] or not
        // if it is marked as #[wraps], it is only a syntax sugar for "has a wrapper field"
        self.definition().0.wraps
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Fields<'a, 'src, L: Layout> {
    pub(super) input: &'src syn::DeriveInput,
    pub(super) ident: &'src syn::Ident,
    pub(super) src: &'src syn::Fields,
    pub(super) parent: DefinitionStructOrVariant<'a, L>,
}

impl<'a, 'src, L: Layout> Fields<'a, 'src, L> {
    #[must_use]
    pub fn definition(&self) -> &DefinitionStructOrVariant<'a, L> {
        &self.parent
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.src.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.src.is_empty()
    }

    #[must_use]
    pub fn iter(&'a self) -> impl Iterator<Item = Field<'a, 'src, L>> + 'a {
        self.src.iter().enumerate().map(move |(index, src)| Field {
            input: self.input,
            src,
            parent: self.parent.clone(),
            index,
        })
    }
}

#[cfg_attr(feature = "debug", derive(Debug))]
pub struct Field<'a, 'src, L: Layout> {
    pub(super) input: &'src syn::DeriveInput,
    pub(super) src: &'src syn::Field,
    pub(super) parent: DefinitionStructOrVariant<'a, L>,
    pub(super) index: usize,
}

impl<'a, L: Layout> Field<'a, '_, L> {
    pub(super) fn definition(&self) -> &DefinitionField<L> {
        &self.parent.fields()[self.index]
    }

    pub fn parent_definition(&self) -> &DefinitionStructOrVariant<'a, L> {
        &self.parent
    }

    pub fn is_wrapper(&self) -> bool {
        self.definition().wraps
    }

    pub fn extra(&self) -> &L::FieldExtra {
        &self.definition().extra
    }
}
