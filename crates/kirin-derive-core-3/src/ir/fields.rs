use super::definition::*;

pub trait HasFields<'src, L: Layout> {
    type Attr;
    #[must_use]
    fn fields(&self) -> Fields<'_, 'src, Self::Attr, L>;
}

impl<'src, L: Layout> HasFields<'src, L> for Struct<'src, L> {
    type Attr = L::StructAttr;
    fn fields(&self) -> Fields<'_, 'src, Self::Attr, L> {
        Fields {
            input: self.input,
            src: &self.src.fields,
            parent: &self.definition.0,
        }
    }
}

impl<'src, L: Layout> HasFields<'src, L> for Variant<'_, 'src, L> {
    type Attr = L::VariantAttr;
    fn fields(&self) -> Fields<'_, 'src, Self::Attr, L> {
        Fields {
            input: self.input,
            src: &self.src.fields,
            parent: &self.parent.variants[self.index].0,
        }
    }
}
