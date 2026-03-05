use super::utils::{renamed_field_idents, tuple_field_idents};

#[derive(Debug, Clone)]
pub struct FieldBindings {
    pub is_tuple: bool,
    pub field_count: usize,
    pub field_idents: Vec<syn::Ident>,
    pub original_field_names: Vec<syn::Ident>,
}

impl FieldBindings {
    pub fn tuple(prefix: &str, count: usize) -> Self {
        Self {
            is_tuple: true,
            field_count: count,
            field_idents: tuple_field_idents(prefix, count),
            original_field_names: Vec::new(),
        }
    }

    pub fn named(prefix: &str, fields: Vec<syn::Ident>) -> Self {
        let count = fields.len();
        let prefixed = renamed_field_idents(&format!("{}_", prefix), &fields);
        Self {
            is_tuple: false,
            field_count: count,
            field_idents: prefixed,
            original_field_names: fields,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.field_count == 0
    }

    pub fn renamed(&self, prefix: &str) -> Vec<syn::Ident> {
        if self.is_tuple {
            tuple_field_idents(prefix, self.field_count)
        } else {
            renamed_field_idents(&format!("{}_", prefix), &self.original_field_names)
        }
    }

    pub fn with_prefix(&self, prefix: &str) -> Self {
        Self {
            is_tuple: self.is_tuple,
            field_count: self.field_count,
            field_idents: self.renamed(prefix),
            original_field_names: self.original_field_names.clone(),
        }
    }
}
