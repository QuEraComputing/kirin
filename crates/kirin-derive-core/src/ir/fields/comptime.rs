use super::index::FieldIndex;
use crate::ir::{attrs::KirinFieldOptions, layout::Layout};
use darling::FromField;

#[derive(Debug, Clone)]
pub struct CompileTimeValues<L: Layout> {
    data: Vec<CompileTimeValue<L>>,
}

impl<L: Layout> Default for CompileTimeValues<L> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl<L: Layout> CompileTimeValues<L> {
    pub fn add(&mut self, index: usize, f: &syn::Field) -> darling::Result<bool> {
        let attrs = KirinFieldOptions::from_field(f)?;
        let extra = L::ExtraFieldAttrs::from_field(f)?;
        self.data.push(CompileTimeValue {
            field: FieldIndex::new(f.ident.clone(), index),
            ty: f.ty.clone(),
            into: attrs.into,
            default: attrs.default,
            extra,
        });
        Ok(true)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CompileTimeValue<L>> {
        self.data.iter()
    }
}

#[derive(Debug, Clone)]
pub struct CompileTimeValue<L: Layout> {
    pub field: FieldIndex,
    pub ty: syn::Type,
    pub into: bool,
    pub default: Option<syn::Expr>,
    pub extra: L::ExtraFieldAttrs,
}
