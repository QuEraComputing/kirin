use super::definition::*;

pub trait HasFields<'src, L: Layout> {
    type Attr;
    #[must_use]
    fn fields(&self) -> Fields<'_, 'src, L>;
}

impl<'src, L: Layout> HasFields<'src, L> for Struct<'src, L> {
    type Attr = L::StructAttr;
    fn fields(&self) -> Fields<'_, 'src, L> {
        Fields {
            input: self.input,
            ident: &self.input.ident,
            src: &self.src.fields,
            parent: (&self.definition).into(),
        }
    }
}

impl<'src, L: Layout> HasFields<'src, L> for Variant<'_, 'src, L> {
    type Attr = L::VariantAttr;
    fn fields(&self) -> Fields<'_, 'src, L> {
        Fields {
            input: self.input,
            ident: &self.src.ident,
            src: &self.src.fields,
            parent: (&self.parent.variants[self.index]).into(),
        }
    }
}
