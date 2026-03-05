use crate::context::{DeriveContext, StatementContext};
use crate::ir::{self, StandardLayout, fields::Collection};
use crate::tokens::{DelegationAssocType, DelegationCall};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

use super::MethodPattern;

/// Which field category to generate iterators for.
#[derive(Clone, Copy, Debug)]
pub enum FieldIterKind {
    Arguments,
    Results,
    Blocks,
    Successors,
    Regions,
}

/// Method pattern that chains field iterators for a given category.
///
/// For wrapper variants, delegates to the wrapped type's trait method.
/// For non-wrapper variants, chains `iter()` / `iter_mut()` calls for
/// each field of the matching category.
pub struct FieldCollection {
    pub field_kind: FieldIterKind,
    pub mutable: bool,
    pub default_crate_path: syn::Path,
    pub trait_path: syn::Path,
    pub trait_lifetime: syn::Lifetime,
    pub trait_method: syn::Ident,
    pub trait_type_iter: syn::Ident,
    pub matching_type: syn::Path,
}

impl FieldCollection {
    pub fn full_trait_path(&self, ctx: &DeriveContext<'_, StandardLayout>) -> syn::Path {
        ctx.meta
            .path_builder(&self.default_crate_path)
            .full_trait_path(&self.trait_path)
    }

    pub fn full_matching_type(&self, ctx: &DeriveContext<'_, StandardLayout>) -> syn::Path {
        ctx.meta
            .path_builder(&self.default_crate_path)
            .full_path(&self.matching_type)
    }

    pub fn matching_item(&self, ctx: &DeriveContext<'_, StandardLayout>) -> TokenStream {
        let lifetime = &self.trait_lifetime;
        let matching_type = self.full_matching_type(ctx);
        if self.mutable {
            quote! { &#lifetime mut #matching_type }
        } else {
            quote! { &#lifetime #matching_type }
        }
    }

    fn fields_for_kind<'a>(
        &self,
        statement: &'a ir::Statement<StandardLayout>,
    ) -> Vec<FieldAccess<'a>> {
        match self.field_kind {
            FieldIterKind::Arguments => statement
                .arguments()
                .map(FieldAccess::from_field_info)
                .collect(),
            FieldIterKind::Results => statement
                .results()
                .map(FieldAccess::from_field_info)
                .collect(),
            FieldIterKind::Blocks => statement
                .blocks()
                .map(FieldAccess::from_field_info)
                .collect(),
            FieldIterKind::Successors => statement
                .successors()
                .map(FieldAccess::from_field_info)
                .collect(),
            FieldIterKind::Regions => statement
                .regions()
                .map(FieldAccess::from_field_info)
                .collect(),
        }
    }

    fn iter_expr(&self, fields: &[FieldAccess<'_>], matching_item: &TokenStream) -> TokenStream {
        let mut expr = None;
        for field in fields {
            let iter = field.iter_expr(self.mutable);
            expr = Some(match expr {
                Some(acc) => quote! { #acc.chain(#iter) },
                None => iter,
            });
        }
        expr.unwrap_or_else(|| quote! { std::iter::empty::<#matching_item>() })
    }

    pub fn iter_type(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        fields: &[FieldAccess<'_>],
        matching_item: &TokenStream,
    ) -> TokenStream {
        let matching_type = self.full_matching_type(ctx);
        let lifetime = &self.trait_lifetime;
        let mut ty = None;
        for field in fields {
            let next_ty = field.iter_type(self.mutable, lifetime, &matching_type, matching_item);
            ty = Some(match ty {
                Some(acc) => quote! { std::iter::Chain<#acc, #next_ty> },
                None => next_ty,
            });
        }
        ty.unwrap_or_else(|| quote! { std::iter::Empty<#matching_item> })
    }

    /// Compute the iterator expression and inner type for a statement.
    pub fn statement_iter(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> (TokenStream, TokenStream) {
        let matching_item = self.matching_item(ctx);
        if stmt_ctx.is_wrapper {
            let wrapper_expr = self.wrapper_expr(ctx, stmt_ctx);
            let wrapper_type = self.wrapper_type(ctx, stmt_ctx);
            (wrapper_expr, wrapper_type)
        } else {
            let fields = self.fields_for_kind(stmt_ctx.stmt);
            let iter_expr = self.iter_expr(&fields, &matching_item);
            let inner_type = self.iter_type(ctx, &fields, &matching_item);
            (iter_expr, inner_type)
        }
    }

    fn wrapper_expr(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> TokenStream {
        let wrapper = stmt_ctx.wrapper.expect("wrapper expected");
        let wrapper_field = {
            let name = wrapper.field.name();
            quote! { #name }
        };
        let wrapper_ty = &wrapper.ty;
        let trait_path = self.full_trait_path(ctx);
        DelegationCall {
            wrapper_ty: quote! { #wrapper_ty },
            trait_path: quote! { #trait_path },
            trait_method: self.trait_method.clone(),
            field: wrapper_field,
        }
        .to_token_stream()
    }

    fn wrapper_type(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> TokenStream {
        let wrapper = stmt_ctx.wrapper.expect("wrapper expected");
        let wrapper_ty = &wrapper.ty;
        let trait_path = self.full_trait_path(ctx);
        let trait_generics = self.trait_generics();
        let (_, trait_ty_generics, _) = trait_generics.split_for_impl();
        DelegationAssocType {
            wrapper_ty: quote! { #wrapper_ty },
            trait_path: quote! { #trait_path },
            trait_generics: quote! { #trait_ty_generics },
            assoc_type_ident: self.trait_type_iter.clone(),
        }
        .to_token_stream()
    }

    pub fn trait_generics(&self) -> syn::Generics {
        let mut generics = syn::Generics::default();
        generics
            .params
            .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                self.trait_lifetime.clone(),
            )));
        generics
    }
}

impl MethodPattern<StandardLayout> for FieldCollection {
    fn for_struct(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        let (iter_expr, _) = self.statement_iter(ctx, stmt_ctx);
        let iter_name = format_ident!(
            "{}{}Iter",
            ctx.meta.name,
            crate::misc::to_camel_case(self.trait_method.to_string()),
        );
        let pattern = &stmt_ctx.pattern;
        if stmt_ctx.pattern.is_empty() {
            Ok(quote! {
                #iter_name {
                    inner: #iter_expr,
                }
            })
        } else {
            Ok(quote! {
                let Self #pattern = self;
                #iter_name {
                    inner: #iter_expr,
                }
            })
        }
    }

    fn for_variant(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        stmt_ctx: &StatementContext<'_, StandardLayout>,
    ) -> darling::Result<TokenStream> {
        let (iter_expr, _) = self.statement_iter(ctx, stmt_ctx);
        let iter_name = format_ident!(
            "{}{}Iter",
            ctx.meta.name,
            crate::misc::to_camel_case(self.trait_method.to_string()),
        );
        let variant_name = &stmt_ctx.stmt.name;
        Ok(quote! { #iter_name::#variant_name(#iter_expr) })
    }
}

/// Helper for accessing a single field in an iterator chain.
pub struct FieldAccess<'a> {
    pub name: TokenStream,
    pub collection: Collection,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> FieldAccess<'a> {
    pub fn from_field_info(field: &'a ir::fields::FieldInfo<StandardLayout>) -> Self {
        let name = match &field.ident {
            Some(ident) => quote! { #ident },
            None => {
                let name = format_ident!("field_{}", field.index);
                quote! { #name }
            }
        };
        Self {
            name,
            collection: field.collection.clone(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn iter_expr(&self, mutable: bool) -> TokenStream {
        let name = &self.name;
        match self.collection {
            Collection::Single => quote! { std::iter::once(#name) },
            Collection::Vec | Collection::Option => {
                if mutable {
                    quote! { #name.iter_mut() }
                } else {
                    quote! { #name.iter() }
                }
            }
        }
    }

    pub fn iter_type(
        &self,
        mutable: bool,
        lifetime: &syn::Lifetime,
        matching_type: &syn::Path,
        matching_item: &TokenStream,
    ) -> TokenStream {
        match self.collection {
            Collection::Single => quote! { std::iter::Once<#matching_item> },
            Collection::Vec => {
                if mutable {
                    quote! { std::slice::IterMut<#lifetime, #matching_type> }
                } else {
                    quote! { std::slice::Iter<#lifetime, #matching_type> }
                }
            }
            Collection::Option => {
                if mutable {
                    quote! { std::option::IterMut<#lifetime, #matching_type> }
                } else {
                    quote! { std::option::Iter<#lifetime, #matching_type> }
                }
            }
        }
    }
}
