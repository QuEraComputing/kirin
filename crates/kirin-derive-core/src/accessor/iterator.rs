use quote::{format_ident, quote};
use syn::spanned::Spanned;

use crate::{DeriveContext, DeriveHelperAttribute, accessor::config::Config};

pub struct IteratorImpl {
    name: syn::Ident,
    generics: syn::Generics,
    iter_name: syn::Ident,
    iter_generics: syn::Generics,
    iter_lifetime: syn::Lifetime,
    matching_type: syn::Ident,
    inner: IteratorImplInner,
}

enum IteratorImplInner {
    Struct(InstructionInfo),
    Enum(syn::Ident, Vec<InstructionInfo>),
}

enum InstructionInfo {
    Anonymous {
        /// The name of the instruction struct/variant
        name: syn::Ident,
        field_count: usize,
        indices: Vec<syn::LitInt>,
    },
    AnonymousContainer(syn::Ident),
    Named {
        /// The name of the instruction
        name: syn::Ident,
        idents: Vec<syn::Ident>,
    },
    NamedContainer(syn::Ident),
    Unit,
    /// Wraps(<variant/struct name>)
    /// wraps another instruction as
    /// Name(OtherInstruction)
    Wraps,
}

impl IteratorImpl {
    pub fn new<A>(config: &Config, ctx: &DeriveContext<A>) -> Self
    where
        A: DeriveHelperAttribute,
    {
        let typename = config.matching_type.to_string();
        match &ctx.input.data {
            syn::Data::Struct(data) => {
                let (iter_generics, iter_lifetime) = Self::generics(&ctx.input.generics);
                Self {
                    name: ctx.input.ident.clone(),
                    iter_name: config.accessor_iter.clone(),
                    iter_generics: iter_generics,
                    iter_lifetime,
                    matching_type: config.matching_type.clone(),
                    generics: ctx.input.generics.clone(),
                    inner: IteratorImplInner::Struct(InstructionInfo::from_fields(
                        ctx.global_wraps(),
                        &ctx.input.ident,
                        &data.fields,
                        &typename,
                    )),
                }
            }
            syn::Data::Enum(data) => {
                let (iter_generics, iter_lifetime) = Self::generics(&ctx.input.generics);
                Self {
                    name: ctx.input.ident.clone(),
                    iter_name: config.accessor_iter.clone(),
                    iter_generics: iter_generics,
                    iter_lifetime,
                    matching_type: config.matching_type.clone(),
                    generics: ctx.input.generics.clone(),
                    inner: IteratorImplInner::Enum(
                        ctx.input.ident.clone(),
                        data.variants
                            .iter()
                            .map(|variant| {
                                InstructionInfo::from_fields(
                                    ctx.global_wraps() || ctx.variant_wraps(&variant.ident),
                                    &variant.ident,
                                    &variant.fields,
                                    &typename,
                                )
                            })
                            .collect(),
                    ),
                }
            }
            _ => panic!("only structs and enums are supported"),
        }
    }

    fn generics(generics: &syn::Generics) -> (syn::Generics, syn::Lifetime) {
        let lifetime = syn::Lifetime::new("'__kirin_ir_iter_a", proc_macro2::Span::call_site());
        let mut g = generics.clone();
        g.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(lifetime.clone())),
        );
        (g, lifetime)
    }

    pub fn generate(&self) -> proc_macro2::TokenStream {
        match &self.inner {
            IteratorImplInner::Struct(InstructionInfo::Wraps) => return quote! {},
            IteratorImplInner::Enum(_, infos) => {
                if infos
                    .iter()
                    .all(|info| matches!(info, InstructionInfo::Wraps))
                {
                    return quote! {};
                }
            }
            _ => {
                // continue
            }
        }

        let body = self.generate_body();
        let name = &self.name;
        let iter_name = self.iter_name.clone();
        let lifetime = &self.iter_lifetime;
        let iter_generics = &self.iter_generics;
        let matching_type = &self.matching_type;
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();
        let (_, ty_generics, _) = self.generics.split_for_impl();
        quote! {
            #[automatically_derived]
            pub struct #iter_name #iter_generics {
                parent: &#lifetime #name #ty_generics,
                index: usize,
            }

            #[automatically_derived]
            impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                type Item = &#lifetime ::kirin_ir::#matching_type;

                fn next(&mut self) -> Option<Self::Item> {
                    #body
                }
            }
        }
    }

    fn generate_body(&self) -> proc_macro2::TokenStream {
        use IteratorImplInner::*;
        match &self.inner {
            Struct(info) => info.struct_body(),
            Enum(name, info) => {
                let matching_arms = info
                    .iter()
                    .map(|variant| variant.variant_matching_arm(&name))
                    .collect::<Vec<_>>();

                quote! {
                    match self.parent {
                        #(#matching_arms)*
                        _ => None,
                    }
                }
            }
        }
    }
}

impl InstructionInfo {
    fn from_fields(wraps: bool, name: &syn::Ident, fields: &syn::Fields, typename: &str) -> Self {
        use syn::Fields::*;
        if wraps {
            match fields {
                Unnamed(fields) if fields.unnamed.len() == 1 => {
                    return InstructionInfo::Wraps;
                }
                _ => {
                    panic!(
                        "`wrap` attribute can only be applied to tuple structs \
                        or variants with a single field"
                    );
                }
            }
        }

        match fields {
            Named(fields_named) => {
                if let Some((_, _)) =
                    has_container_and_only_one_container(&fields_named.named, typename)
                {
                    Self::NamedContainer(name.clone())
                } else {
                    Self::Named {
                        name: name.clone(),
                        idents: fields_named
                            .named
                            .iter()
                            .filter_map(|field| {
                                if is_type(&field.ty, typename) {
                                    Some(field.ident.clone().unwrap())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<syn::Ident>>(),
                    }
                }
            }
            Unnamed(fields_unnamed) => {
                if let Some((_, _)) =
                    has_container_and_only_one_container(&fields_unnamed.unnamed, typename)
                {
                    Self::AnonymousContainer(name.clone())
                } else {
                    Self::Anonymous {
                name: name.clone(),
                field_count: fields_unnamed.unnamed.len(),
                indices: fields_unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .filter_map(|(i, field)| {
                        if is_type(&field.ty, typename) {
                            Some(syn::LitInt::new(&i.to_string(), field.span()))
                        } else if is_type_in_generic(&field.ty, typename) {
                            panic!(
                                "Generic field types like Vec<{}> are not supported yet, consider implementing manually",
                                typename
                            );
                        } else {
                            None
                        }
                    })
                    .collect(),
            }
                }
            }
            Unit => InstructionInfo::Unit,
        }
    }

    fn struct_body(&self) -> proc_macro2::TokenStream {
        use InstructionInfo::*;
        match self {
            Anonymous {
                name,
                field_count,
                indices: _,
            } => {
                let vars = (0..*field_count)
                    .map(|i| format_ident!("__self_{}", i))
                    .collect::<Vec<_>>();
                let arms = self.iteration_matching_arms();
                quote! {
                    let #name (#(#vars),*) = self.parent;
                    match self.index {
                        #(#arms)*
                        _ => None,
                    }
                }
            }
            Named { name, idents } => {
                let arms = self.iteration_matching_arms();
                quote! {
                    let #name { #(#idents,)* .. } = self.parent;
                    match self.index {
                        #(#arms)*
                        _ => None,
                    }
                }
            }
            AnonymousContainer(_) | NamedContainer(_) => {
                quote! {
                    unreachable!("struct with container should not generate iterator");
                }
            }
            Unit => {
                quote! {
                    None
                }
            }
            Wraps => {
                panic!("Wraps variant should not generate any iterator");
            }
        }
    }

    fn variant_matching_arm(&self, name: &syn::Ident) -> proc_macro2::TokenStream {
        use InstructionInfo::*;
        match self {
            Anonymous {
                name: variant_name,
                field_count,
                indices,
            } => {
                // happy path: no fields matched
                if indices.is_empty() {
                    return quote! {};
                }

                let vars = (0..*field_count)
                    .map(|i| format_ident!("__self_{}", i))
                    .collect::<Vec<_>>();
                let arms = self.iteration_matching_arms();
                quote! {
                    #name::#variant_name(#(#vars),*) => {
                        match self.index {
                            #(#arms)*
                            _ => None,
                        }
                    }
                }
            }
            Named {
                name: variant_name,
                idents,
            } => {
                // happy path: no fields matched
                if idents.is_empty() {
                    return quote! {};
                }

                let arms = self.iteration_matching_arms();
                quote! {
                    #name::#variant_name { #(#idents,)* .. } => {
                        match self.index {
                            #(#arms)*
                            _ => None,
                        }
                    }
                }
            }
            AnonymousContainer(variant_name) | NamedContainer(variant_name) => {
                quote! {
                    #name::#variant_name => {
                        unreachable!("variant with container should not generate iterator");
                    }
                }
            }
            _ => {
                quote! {}
            }
        }
    }

    fn iteration_matching_arms(&self) -> Vec<proc_macro2::TokenStream> {
        use InstructionInfo::*;
        match self {
            Anonymous { indices, .. } => indices
                .iter()
                .enumerate()
                .map(|(i, i_lit)| {
                    let self_i = format_ident!("__self_{}", i_lit.base10_parse::<usize>().unwrap());
                    quote! {
                        #i => {
                            self.index += 1;
                            Some(#self_i)
                        },
                    }
                })
                .collect(),
            Named { idents, .. } => idents
                .iter()
                .enumerate()
                .map(|(i, ident)| {
                    let i_lit = syn::LitInt::new(&i.to_string(), ident.span());
                    quote! {
                        #i_lit => {
                            self.index += 1;
                            Some(#ident)
                        },
                    }
                })
                .collect(),
            _ => {
                vec![]
            }
        }
    }
}

fn is_type(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(type_path) if type_path.path.is_ident(name))
}

fn is_type_in_generic(ty: &syn::Type, name: &str) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
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
    false
}

pub(super) fn has_container_and_only_one_container(
    field: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    typename: &str,
) -> Option<(usize, Option<syn::Ident>)> {
    let mut found = None;
    for (i, field) in field.iter().enumerate() {
        if is_type_in_generic(&field.ty, typename) {
            if found.is_some() {
                panic!(
                    "Multiple container fields like Vec<{}> are not supported yet, consider implementing manually",
                    typename
                );
            } else {
                found = Some((i, field.ident.clone()));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use syn::{DataStruct, Fields};

    use super::*;

    #[test]
    fn test_is_type() {
        let ty: syn::Type = syn::parse_str("SSAValue").unwrap();
        assert!(is_type(&ty, "SSAValue"));
        assert!(!is_type(&ty, "ResultValue"));
    }

    #[test]
    fn test_is_type_in_generic() {
        let ty: syn::Type = syn::parse_str("Vec<SSAValue>").unwrap();
        assert!(is_type_in_generic(&ty, "SSAValue"));
        assert!(!is_type_in_generic(&ty, "ResultValue"));
    }

    #[test]
    fn test_has_container_and_only_one_container() {
        let fields: syn::DeriveInput = syn::parse_str(
            "struct Foo {
                args: Vec<SSAValue>,
                results: Vec<ResultValue>,
                containers: Vec<ContainerValue>,
            }",
        )
        .unwrap();
        let fields = if let syn::Data::Struct(DataStruct { struct_token: _, fields: Fields::Named(f), semi_token: _ }) = fields.data {
            f
        } else {
            panic!("expected struct");
        };
        let result = has_container_and_only_one_container(&fields.named, "SSAValue");
        assert_eq!(result, Some((0, Some(syn::Ident::new("args", proc_macro2::Span::call_site())))));

        let result = has_container_and_only_one_container(&fields.named, "ResultValue");
        assert_eq!(result, Some((1, Some(syn::Ident::new("results", proc_macro2::Span::call_site())))));

        let result = has_container_and_only_one_container(&fields.named, "ContainerValue");
        assert_eq!(result, Some((2, Some(syn::Ident::new("containers", proc_macro2::Span::call_site())))));
    }
}
