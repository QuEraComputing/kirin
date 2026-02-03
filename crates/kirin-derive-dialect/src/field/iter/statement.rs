use crate::field::iter::context::{DeriveFieldIter, FieldIterKind};
use crate::field::iter::helpers::{FieldInputBuilder, field_name_tokens};
use kirin_derive_core::prelude::*;
use kirin_derive_core::tokens::{FieldPatternTokens, WrapperCallTokens, WrapperIterTypeTokens};
use quote::{ToTokens, format_ident, quote};

/// Generate field name tokens from FieldInfo (for pattern matching in generated code).
fn field_name_tokens_from_info(field: &ir::fields::FieldInfo<StandardLayout>) -> proc_macro2::TokenStream {
    match &field.ident {
        Some(ident) => quote! { #ident },
        None => {
            // For tuple fields, generate `field_0`, `field_1`, etc.
            let name = format_ident!("field_{}", field.index);
            quote! { #name }
        }
    }
}

pub(crate) struct StatementBuilder<'a> {
    ctx: &'a DeriveFieldIter,
    input: &'a InputContext,
}

impl<'a> StatementBuilder<'a> {
    pub(crate) fn new(ctx: &'a DeriveFieldIter, input: &'a InputContext) -> Self {
        Self { ctx, input }
    }

    pub(crate) fn statement_pattern(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> FieldPatternTokens {
        let fields = self.all_fields(statement);
        if fields.is_empty() {
            return FieldPatternTokens::new(false, Vec::new());
        }
        let named = fields.iter().any(|field| field.ident.is_some());
        let names: Vec<proc_macro2::TokenStream> = fields.iter().map(field_name_tokens).collect();
        FieldPatternTokens::new(named, names)
    }

    pub(crate) fn statement_iter_expr(
        &self,
        statement: &ir::Statement<StandardLayout>,
        matching_item: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let fields = self.fields_for_kind(statement);
        self.iter_expr(&fields, matching_item)
    }

    pub(crate) fn statement_iter_type(
        &self,
        statement: &ir::Statement<StandardLayout>,
        matching_item: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let fields = self.fields_for_kind(statement);
        self.iter_type(&fields, matching_item)
    }

    pub(crate) fn statement_wrapper_expr(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> proc_macro2::TokenStream {
        let wrapper = statement.wraps.as_ref().expect("wrapper expected");
        let wrapper_field = field_name_tokens(&wrapper.field);
        let wrapper_ty = &wrapper.ty;
        let trait_path = &self.ctx.trait_path;
        let trait_method = self.ctx.trait_method.clone();
        WrapperCallTokens::builder()
            .wrapper_ty(wrapper_ty)
            .trait_path(trait_path)
            .trait_method(trait_method)
            .field(wrapper_field)
            .build()
            .to_token_stream()
    }

    pub(crate) fn statement_wrapper_type(
        &self,
        statement: &ir::Statement<StandardLayout>,
    ) -> proc_macro2::TokenStream {
        let input_builder = FieldInputBuilder::new(self.ctx, self.input);
        let wrapper = statement.wraps.as_ref().expect("wrapper expected");
        let wrapper_ty = &wrapper.ty;
        let trait_path = &self.ctx.trait_path;
        let trait_type_iter = self.ctx.trait_type_iter.clone();
        if self.input.is_enum {
            let trait_generics = input_builder.trait_generics();
            let (_, trait_ty_generics, _) = trait_generics.split_for_impl();
            WrapperIterTypeTokens::builder()
                .wrapper_ty(wrapper_ty)
                .trait_path(trait_path)
                .trait_generics(trait_ty_generics)
                .assoc_type_ident(trait_type_iter)
                .build()
                .to_token_stream()
        } else {
            let (_, wrapper_ty_generics, _) = self.input.generics.split_for_impl();
            WrapperIterTypeTokens::builder()
                .wrapper_ty(wrapper_ty)
                .trait_path(trait_path)
                .trait_generics(wrapper_ty_generics)
                .assoc_type_ident(trait_type_iter)
                .build()
                .to_token_stream()
        }
    }

    fn fields_for_kind<'b>(
        &self,
        statement: &'b ir::Statement<StandardLayout>,
    ) -> Vec<FieldAccess<'b>> {
        match self.ctx.field_kind {
            FieldIterKind::Arguments => statement
                .arguments()
                .map(|f| FieldAccess::from_field_info(f))
                .collect(),
            FieldIterKind::Results => statement
                .results()
                .map(|f| FieldAccess::from_field_info(f))
                .collect(),
            FieldIterKind::Blocks => statement
                .blocks()
                .map(|f| FieldAccess::from_field_info(f))
                .collect(),
            FieldIterKind::Successors => statement
                .successors()
                .map(|f| FieldAccess::from_field_info(f))
                .collect(),
            FieldIterKind::Regions => statement
                .regions()
                .map(|f| FieldAccess::from_field_info(f))
                .collect(),
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

    fn iter_expr(
        &self,
        fields: &[FieldAccess<'_>],
        matching_item: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let mut expr = None;
        for field in fields {
            let iter = field.iter_expr(self.ctx.mutable);
            expr = Some(match expr {
                Some(acc) => quote! { #acc.chain(#iter) },
                None => iter,
            });
        }
        expr.unwrap_or_else(|| quote! { std::iter::empty::<#matching_item>() })
    }

    fn iter_type(
        &self,
        fields: &[FieldAccess<'_>],
        matching_item: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let mut ty = None;
        for field in fields {
            let next_ty = field.iter_type(self.ctx, self.input, matching_item);
            ty = Some(match ty {
                Some(acc) => quote! { std::iter::Chain<#acc, #next_ty> },
                None => next_ty,
            });
        }
        ty.unwrap_or_else(|| quote! { std::iter::Empty<#matching_item> })
    }
}

struct FieldAccess<'a> {
    name: proc_macro2::TokenStream,
    collection: ir::fields::Collection,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> FieldAccess<'a> {
    fn from_field_info(field: &'a ir::fields::FieldInfo<StandardLayout>) -> Self {
        let name = field_name_tokens_from_info(field);
        Self {
            name,
            collection: field.collection.clone(),
            _phantom: std::marker::PhantomData,
        }
    }

    fn iter_expr(&self, mutable: bool) -> proc_macro2::TokenStream {
        let name = &self.name;
        match self.collection {
            ir::fields::Collection::Single => quote! { std::iter::once(#name) },
            ir::fields::Collection::Vec => {
                if mutable {
                    quote! { #name.iter_mut() }
                } else {
                    quote! { #name.iter() }
                }
            }
            ir::fields::Collection::Option => {
                if mutable {
                    quote! { #name.iter_mut() }
                } else {
                    quote! { #name.iter() }
                }
            }
        }
    }

    fn iter_type(
        &self,
        ctx: &DeriveFieldIter,
        input: &InputContext,
        matching_item: &proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let lifetime = &ctx.trait_lifetime;
        let matching_type = FieldInputBuilder::new(ctx, input).full_matching_type();
        match self.collection {
            ir::fields::Collection::Single => quote! { std::iter::Once<#matching_item> },
            ir::fields::Collection::Vec => {
                if ctx.mutable {
                    quote! { std::slice::IterMut<#lifetime, #matching_type> }
                } else {
                    quote! { std::slice::Iter<#lifetime, #matching_type> }
                }
            }
            ir::fields::Collection::Option => {
                if ctx.mutable {
                    quote! { std::option::IterMut<#lifetime, #matching_type> }
                } else {
                    quote! { std::option::Iter<#lifetime, #matching_type> }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::iter::context::DeriveFieldIter;

    #[test]
    fn test_statement_pattern_named() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = L)]
            struct Example {
                a: SSAValue,
                b: SSAValue,
            }
        };
        let input = ir::Input::<StandardLayout>::from_derive_input(&input).unwrap();
        let ir::Data::Struct(data) = &input.data else {
            panic!("expected struct");
        };
        let mut derive = DeriveFieldIter::new(
            FieldIterKind::Arguments,
            false,
            "::kirin::ir",
            "HasArguments",
            "SSAValue",
            "arguments",
            "Iter",
        );
        derive.input = Some(InputContext::from_input(&input));
        let builder = StatementBuilder::new(&derive, derive.input.as_ref().unwrap());
        let pattern = builder.statement_pattern(&data.0);
        assert!(!pattern.is_empty());
        let tokens = pattern.to_token_stream().to_string();
        assert!(tokens.contains("a"));
        assert!(tokens.contains("b"));
    }
}
