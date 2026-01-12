use std::collections::HashMap;

use kirin_derive_core_2::ir::Statement;

use crate::{
    parse::{Format, FormatElement, FormatOption},
    ChumskyLayout,
};

#[derive(Clone, Default)]
pub struct FormatUsage {
    has_format: bool,
    entries: HashMap<usize, Vec<FormatOption>>,
}

impl FormatUsage {
    pub fn new(has_format: bool, entries: HashMap<usize, Vec<FormatOption>>) -> Self {
        Self { has_format, entries }
    }

    pub fn has_format(&self) -> bool {
        self.has_format
    }

    pub fn for_index(&self, index: usize) -> &[FormatOption] {
        self.entries
            .get(&index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

pub fn build_format_usage(stmt: &Statement<ChumskyLayout>, format: &Format<'_>) -> FormatUsage {
    let mut map_by_ident = HashMap::new();
    for field in all_fields(stmt) {
        if let Some(ident) = &field.ident {
            map_by_ident.insert(ident.to_string(), field.index);
        }
    }

    let mut entries: HashMap<usize, Vec<FormatOption>> = HashMap::new();
    for elem in format.elements() {
        if let FormatElement::Field(name, opt) = elem {
            let index = name
                .parse::<usize>()
                .ok()
                .or_else(|| map_by_ident.get(*name).copied());
            if let Some(index) = index {
                entries.entry(index).or_default().push(opt.clone());
            }
        }
    }

    FormatUsage::new(true, entries)
}

fn all_fields(
    stmt: &Statement<ChumskyLayout>,
) -> impl Iterator<Item = &kirin_derive_core_2::ir::fields::FieldIndex> {
    stmt.arguments
        .iter()
        .map(|a| &a.field)
        .chain(stmt.results.iter().map(|r| &r.field))
        .chain(stmt.blocks.iter().map(|b| &b.field))
        .chain(stmt.successors.iter().map(|s| &s.field))
        .chain(stmt.regions.iter().map(|r| &r.field))
        .chain(stmt.values.iter().map(|v| &v.field))
}
