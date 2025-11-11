use proc_macro2::TokenStream;
use quote::{format_ident, quote};

#[derive(Clone)]
pub struct AccessorInfo {
    /// name of the accessor method
    pub name: syn::Ident,
    /// name of the iterator type
    pub iter_name: syn::Ident,
    /// matching type
    pub matching_type: syn::Ident,
    /// trait path
    pub trait_path: syn::Path,
    /// lifetime of the accessor trait
    pub lifetime: syn::Lifetime,
}

impl AccessorInfo {
    pub fn new(
        name: impl AsRef<str>,
        matching_type: impl AsRef<str>,
        trait_path: impl AsRef<str>,
    ) -> Self {
        let name_str = name.as_ref();
        let iter_name_str = format!("__Kirin{}Iter", to_camel_case(name_str));
        Self {
            name: syn::Ident::new(name_str, proc_macro2::Span::call_site()),
            iter_name: syn::Ident::new(&iter_name_str, proc_macro2::Span::call_site()),
            matching_type: syn::Ident::new(matching_type.as_ref(), proc_macro2::Span::call_site()),
            trait_path: syn::parse_str(trait_path.as_ref()).unwrap(),
            lifetime: syn::Lifetime::new("'a", proc_macro2::Span::call_site()),
        }
    }

    pub fn generics<'a>(&'a self, generics: &'a syn::Generics) -> AccessorGenerics<'a> {
        let mut g = generics.clone();
        g.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(self.lifetime.clone())),
        );
        AccessorGenerics {
            generics: g,
            input_generics: generics,
            lifetime: &self.lifetime,
        }
    }
}

pub struct AccessorGenerics<'a> {
    pub generics: syn::Generics,
    pub input_generics: &'a syn::Generics,
    pub lifetime: &'a syn::Lifetime,
}

impl<'a> AccessorGenerics<'a> {
    pub fn split_for_impl(
        &self,
    ) -> (
        syn::TypeGenerics,
        syn::Lifetime,
        syn::ImplGenerics,
        syn::TypeGenerics,
        Option<&syn::WhereClause>,
    ) {
        let (impl_generics, type_generics, where_clause) = self.generics.split_for_impl();
        let (_, input_type_generics, _) = self.input_generics.split_for_impl();
        (
            input_type_generics,
            self.lifetime.clone(),
            impl_generics,
            type_generics,
            where_clause,
        )
    }
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

pub fn has_attr(attrs: &[syn::Attribute], attr_name: &str, option: &str) -> bool {
    let mut has_option = false;
    for attr in attrs {
        if attr.path().is_ident(attr_name) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident(option) {
                    has_option = true;
                }
                Ok(())
            })
            .unwrap();
        }
    }
    has_option
}

pub fn is_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident(name))
}

pub fn is_vec_type<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I> + PartialEq<str>,
{
    is_type_in(ty, name, |seg| seg.ident == "Vec")
}

pub fn is_type_in_generic<I>(ty: &syn::Type, name: &I) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
{
    is_type_in(ty, name, |_| true)
}

pub fn is_type_in<I, F>(ty: &syn::Type, name: &I, f: F) -> bool
where
    I: ?Sized,
    syn::Ident: PartialEq<I>,
    F: Fn(&syn::PathSegment) -> bool,
{
    if let syn::Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if f(seg) {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for each in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = &each {
                            if is_type(inner_ty, name) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

pub struct NamedMatchedFields {
    matching_type: syn::Ident,
    matching_fields: Vec<NamedMatchedField>,
}

impl NamedMatchedFields {
    pub fn new(info: &AccessorInfo, fields: &syn::FieldsNamed) -> Self {
        Self {
            matching_type: info.matching_type.clone(),
            matching_fields: fields
                .named
                .iter()
                .filter_map(|f| NamedMatchedField::try_from_field(f, &info.matching_type))
                .collect(),
        }
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchedField::One(ident) => ident.clone(),
                NamedMatchedField::Vec(ident) => ident.clone(),
            })
            .collect()
    }

    pub fn iter(&self) -> TokenStream {
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchedField::One(ident) => quote! { std::iter::once(#ident) },
                NamedMatchedField::Vec(ident) => quote! { #ident.iter() },
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote! { #acc.chain(#field) })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or(quote! { std::iter::empty() })
    }

    pub fn iter_type(&self) -> TokenStream {
        let matching_type = &self.matching_type;
        self.matching_fields
            .iter()
            .map(|f| match f {
                NamedMatchedField::One(_) => quote! { std::iter::Once<&'a #matching_type> },
                NamedMatchedField::Vec(_) => quote! { std::slice::Iter<'a, #matching_type> },
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote! { std::iter::Chain<#acc, #field> })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or(quote! { std::iter::Empty<&#matching_type> })
    }
}

pub struct UnnamedMatchedFields {
    nfields: usize,
    matching_type: syn::Ident,
    matching_fields: Vec<UnnamedMatchedField>,
}

impl UnnamedMatchedFields {
    pub fn new(info: &AccessorInfo, fields: &syn::FieldsUnnamed) -> Self {
        Self {
            nfields: fields.unnamed.len(),
            matching_type: info.matching_type.clone(),
            matching_fields: fields
                .unnamed
                .iter()
                .enumerate()
                .filter_map(|(i, f)| UnnamedMatchedField::try_from_field(i, f, &info.matching_type))
                .collect(),
        }
    }

    pub fn vars(&self) -> Vec<syn::Ident> {
        (0..self.nfields)
            .map(|i| format_ident!("field_{}", i))
            .collect()
    }

    pub fn iter(&self) -> TokenStream {
        self.matching_fields
            .iter()
            .map(|f| match f {
                UnnamedMatchedField::One(index) => {
                    let var = format_ident!("field_{}", index);
                    quote::quote! { std::iter::once(#var) }
                }
                UnnamedMatchedField::Vec(index) => {
                    let var = format_ident!("field_{}", index);
                    quote::quote! { #var.iter() }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote::quote! { #acc.chain(#field) })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or_else(|| quote::quote! { std::iter::empty() })
    }

    pub fn iter_type(&self) -> TokenStream {
        let matching_type = &self.matching_type;
        self.matching_fields
            .iter()
            .map(|f| match f {
                UnnamedMatchedField::One(_) => {
                    quote::quote! { std::iter::Once<&'a #matching_type> }
                }
                UnnamedMatchedField::Vec(_) => {
                    quote::quote! { std::slice::Iter<'a, #matching_type> }
                }
            })
            .fold(None, |acc: Option<TokenStream>, field| {
                if let Some(acc) = acc {
                    Some(quote::quote! { std::iter::Chain<#acc, #field> })
                } else {
                    Some(field.clone())
                }
            })
            .unwrap_or_else(|| quote::quote! { std::iter::Empty<&#matching_type> })
    }
}

enum NamedMatchedField {
    One(syn::Ident),
    Vec(syn::Ident),
}

enum UnnamedMatchedField {
    One(usize),
    Vec(usize),
}

impl NamedMatchedField {
    fn try_from_field(f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(NamedMatchedField::One(f.ident.clone().unwrap()))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(NamedMatchedField::Vec(f.ident.clone().unwrap()))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}

impl UnnamedMatchedField {
    fn try_from_field(index: usize, f: &syn::Field, matching_type: &syn::Ident) -> Option<Self> {
        if is_type(&f.ty, matching_type) {
            Some(UnnamedMatchedField::One(index))
        } else if is_vec_type(&f.ty, matching_type) {
            Some(UnnamedMatchedField::Vec(index))
        } else if is_type_in_generic(&f.ty, matching_type) {
            panic!("generic types other than Vec are not supported");
        } else {
            None
        }
    }
}
