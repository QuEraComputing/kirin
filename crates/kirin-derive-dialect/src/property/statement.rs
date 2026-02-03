use crate::property::context::{DeriveProperty, InputContext};
use kirin_derive_core::prelude::*;
use kirin_derive_core::tokens::{FieldPatternTokens, WrapperCallTokens};
use quote::{ToTokens, quote};

#[derive(Clone, Debug)]
pub(crate) struct StatementInfo {
    pub(crate) name: syn::Ident,
    pub(crate) pattern: FieldPatternTokens,
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
    ) -> FieldPatternTokens {
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
            return WrapperCallTokens::builder()
                .wrapper_ty(wrapper_ty)
                .trait_path(trait_path)
                .trait_method(derive.trait_method.clone())
                .field(wrapper_field)
                .build()
                .to_token_stream();
        }

        let glob = input.global_value;
        if input.core.is_enum {
            let stmt = derive.kind.statement_value(statement);
            quote! { #glob || #stmt }
        } else {
            quote! { #glob }
        }
    }

    fn all_fields(&self, statement: &ir::Statement<StandardLayout>) -> Vec<ir::fields::FieldIndex> {
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

    fn field_pattern(&self, fields: &[ir::fields::FieldIndex]) -> FieldPatternTokens {
        if fields.is_empty() {
            return FieldPatternTokens::new(false, Vec::new());
        }
        let named = fields.iter().any(|field| field.ident.is_some());
        let names: Vec<proc_macro2::TokenStream> = fields.iter().map(field_name_tokens).collect();
        FieldPatternTokens::new(named, names)
    }
}

fn field_name_tokens(field: &ir::fields::FieldIndex) -> proc_macro2::TokenStream {
    let name = field.name();
    quote! { #name }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::context::{DeriveProperty, InputContext, PropertyKind};
    use kirin_derive_core::derive::InputMeta as CoreInputMeta;

    #[test]
    fn test_statement_pattern_unnamed() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = L)]
            struct Example(SSAValue, ResultValue);
        };
        let input = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let ir::Data::Struct(data) = &input.data else {
            panic!("expected struct");
        };
        let mut derive = DeriveProperty::new(
            PropertyKind::Pure,
            "::kirin::ir",
            "IsPure",
            "is_pure",
            "bool",
        );
        derive.input = Some(InputContext {
            core: CoreInputMeta::from_input(&input),
            global_value: false,
        });
        let builder = StatementBuilder::new();
        let pattern = builder.statement_pattern(&data.0);
        assert!(!pattern.is_empty());
        let tokens = pattern.to_token_stream().to_string();
        assert!(tokens.contains("field_0"));
        assert!(tokens.contains("field_1"));
    }
}
