use crate::generators::property::context::{DeriveProperty, InputContext};
use crate::prelude::*;
use crate::tokens::{DelegationCall, Pattern};
use quote::{ToTokens, quote};

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) pattern: Pattern,
    pub(crate) pattern_empty: bool,
    pub(crate) value_expr: proc_macro2::TokenStream,
    pub(crate) is_wrapper: bool,
}

pub(crate) struct StatementBuilder;

impl StatementBuilder {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn statement_pattern(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> Pattern {
        let fields = self.all_fields(statement);
        self.field_pattern(&fields)
    }

    pub(crate) fn statement_value_expr(
        &self,
        derive: &DeriveProperty,
        input: &InputContext,
        statement: &ir::Statement<StandardLayout>,
    ) -> proc_macro2::TokenStream {
        if let Some(wrapper) = &statement.wraps {
            let wrapper_field = field_name_tokens(&wrapper.field);
            let wrapper_ty = &wrapper.ty;
            let trait_path = derive.full_trait_path(input);
            return DelegationCall {
                wrapper_ty: quote! { #wrapper_ty },
                trait_path: quote! { #trait_path },
                trait_method: derive.trait_method.clone(),
                field: wrapper_field,
            }
            .to_token_stream();
        }

        let glob = input.global_value;
        if input.core.is_enum {
            let stmt = derive.reader.statement_value(statement);
            quote! { #glob || #stmt }
        } else {
            quote! { #glob }
        }
    }

    fn all_fields(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> Vec<ir::fields::FieldIndex> {
        let mut fields = Vec::new();
        if let Some(wrapper) = &statement.wraps {
            fields.push(wrapper.field.clone());
        }
        for f in statement.iter_all_fields() {
            fields.push(ir::fields::FieldIndex::new(f.ident.clone(), f.index));
        }
        fields.sort_by_key(|field| field.index);
        fields
    }

    fn field_pattern(&self, fields: &[ir::fields::FieldIndex]) -> Pattern {
        if fields.is_empty() {
            return Pattern::new(false, Vec::new());
        }
        let named = fields.iter().any(|field| field.ident.is_some());
        let names: Vec<proc_macro2::TokenStream> = fields.iter().map(field_name_tokens).collect();
        Pattern::new(named, names)
    }
}

fn field_name_tokens(field: &ir::fields::FieldIndex) -> proc_macro2::TokenStream {
    let name = field.name();
    quote! { #name }
}
