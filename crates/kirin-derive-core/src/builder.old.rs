use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::{data::*, utils::*};

pub struct BuilderInfo {
    trait_path: syn::Path,
    generics: syn::Generics,
    method_name: syn::Ident,
}

impl<'input> TraitInfo<'input> for BuilderInfo {
    type GlobalAttributeData = ();
    type MatchingFields = Fields;

    fn default_crate_path(&self) -> syn::Path {
        syn::parse_quote! { ::kirin::ir }
    }

    fn method_name(&self) -> &syn::Ident {
        &self.method_name
    }

    fn relative_trait_path(&self) -> &syn::Path {
        &self.trait_path
    }

    fn trait_generics(&self) -> &syn::Generics {
        &self.generics
    }
}

pub enum Fields {
    Named(Option<syn::Ident>, Vec<NamedField>),
    Unnamed(Option<syn::Ident>, Vec<UnnamedField>),
    Unit,
}

impl Fields {
    pub fn builder(&self) -> Option<&syn::Ident> {
        match self {
            Fields::Named(builder, _) => builder.as_ref(),
            Fields::Unnamed(builder, _) => builder.as_ref(),
            Fields::Unit => None,
        }
    }

    pub fn from_fields(builder: Option<syn::Ident>, fields: &syn::Fields) -> Self {
        match fields {
            syn::Fields::Named(named) => Self::Named(
                builder,
                named
                    .named
                    .iter()
                    .map(|f| NamedField {
                        name: f.ident.clone().unwrap(),
                        ty: f.ty.clone(),
                        kirin_ty: KirinAttribute::from_field_attrs(&f.attrs).ty,
                    })
                    .collect(),
            ),
            syn::Fields::Unnamed(unnamed) => Self::Unnamed(
                builder,
                unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| UnnamedField {
                        index: i,
                        ty: f.ty.clone(),
                        kirin_ty: KirinAttribute::from_field_attrs(&f.attrs).ty,
                    })
                    .collect(),
            ),
            syn::Fields::Unit => Self::Unit,
        }
    }
}

impl<'input> FromStructFields<'input, BuilderInfo> for Fields {
    fn from_struct_fields(
        ctx: &Context<'input, BuilderInfo>,
        _parent: &'input syn::DataStruct,
        fields: &'input syn::Fields,
    ) -> Self {
        Fields::from_fields(ctx.kirin_attr.builder_fn.clone(), fields)
    }
}

impl<'input> FromVariantFields<'input, BuilderInfo> for Fields {
    fn from_variant_fields(
        ctx: &Context<'input, BuilderInfo>,
        parent: &'input syn::Variant,
        fields: &'input syn::Fields,
    ) -> Self {
        if ctx.kirin_attr.builder_fn.is_some() {
            panic!("global `#[kirin(builder = ...)]` attribute is not supported on enum variants");
        }
        Fields::from_fields(KirinAttribute::from_attrs(&parent.attrs).builder_fn, fields)
    }
}

pub struct NamedField {
    pub name: syn::Ident,
    pub ty: syn::Type,
    pub kirin_ty: Option<syn::Expr>,
}

impl NamedField {
    pub fn input_name(&self) -> syn::Ident {
        self.name.clone()
    }
}

pub struct UnnamedField {
    pub index: usize,
    pub ty: syn::Type,
    pub kirin_ty: Option<syn::Expr>,
}

impl UnnamedField {
    pub fn input_name(&self) -> syn::Ident {
        format_ident!("arg{}", self.index)
    }
}

impl GenerateFrom<'_, WrapperStruct<'_, Self>> for BuilderInfo {
    fn generate_from(&self, _data: &WrapperStruct<'_, Self>) -> TokenStream {
        quote! {}
    }
}

impl GenerateFrom<'_, RegularStruct<'_, Self>> for BuilderInfo {
    fn generate_from(&self, data: &RegularStruct<'_, Self>) -> TokenStream {
        let name = &data.ctx.input.ident;
        let (impl_generics, ty_generics, where_clause) =
            data.ctx.input.generics.split_for_impl();
        let builder_name = data.fields.builder().cloned().unwrap_or_else(|| {
            format_ident!("op_{}", to_snake_case(name.to_string()))
        });
        let crate_path = data.ctx.kirin_attr.crate_path.clone().unwrap_or_else(|| {
            syn::parse_quote! { ::kirin::ir }
        });

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                pub fn #builder_name<L>(arena: &mut #crate_path::Arena<L>) -> Self
                where
                    L: Language + From<#name #ty_generics>,
                {
                    let parent = arena.new_statement_id();
                    Self {}
                }
            }
        }
    }
}
