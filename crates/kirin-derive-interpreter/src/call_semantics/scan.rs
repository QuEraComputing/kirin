use super::{DeriveCallSemantics, InputContext, StatementInfo};
use kirin_derive_core::derive::InputMeta;
use kirin_derive_core::prelude::*;
use kirin_derive_core::tokens::FieldPatternTokens;
use quote::quote;

impl<'ir> Scan<'ir, StandardLayout> for DeriveCallSemantics {
    fn scan_input(&mut self, input: &'ir ir::Input<StandardLayout>) -> darling::Result<()> {
        self.input = Some(InputContext {
            core: InputMeta::from_input(input),
        });
        self.statements.clear();
        scan::scan_input(self, input)
    }

    fn scan_statement(
        &mut self,
        statement: &'ir ir::Statement<StandardLayout>,
    ) -> darling::Result<()> {
        let is_wrapper = statement.wraps.is_some();
        let wrapper_ty = statement.wraps.as_ref().map(|w| w.ty.clone());
        let wrapper_binding = statement.wraps.as_ref().map(|w| {
            let name = w.field.name();
            quote! { #name }
        });

        let pattern = build_pattern(statement);

        let info = StatementInfo {
            name: statement.name.clone(),
            is_wrapper,
            wrapper_ty,
            wrapper_binding,
            pattern,
        };
        self.statements.insert(statement.name.to_string(), info);
        Ok(())
    }
}

fn build_pattern(statement: &ir::Statement<StandardLayout>) -> FieldPatternTokens {
    let mut fields = Vec::new();
    if let Some(wrapper) = &statement.wraps {
        fields.push(wrapper.field.clone());
    }
    for f in statement.iter_all_fields() {
        fields.push(ir::fields::FieldIndex::new(f.ident.clone(), f.index));
    }
    fields.sort_by_key(|field| field.index);

    if fields.is_empty() {
        return FieldPatternTokens::new(false, Vec::new());
    }
    let named = fields.iter().any(|f| f.ident.is_some());
    let names: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .map(|f| {
            let name = f.name();
            quote! { #name }
        })
        .collect();
    FieldPatternTokens::new(named, names)
}
