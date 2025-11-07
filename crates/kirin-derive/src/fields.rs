use quote::{format_ident, quote};
use syn::spanned::Spanned;

pub enum FieldInfo {
    Anonymous(syn::Ident, usize, Vec<syn::LitInt>),
    Named(syn::Ident, Vec<syn::Ident>),
    Unit,
}

pub enum DataInfo {
    Struct(FieldInfo),
    Enum(Vec<FieldInfo>),
}

pub struct FilteredFieldInfo {
    pub name: syn::Ident,
    pub generics: syn::Generics,
    pub data: DataInfo,
}

impl FilteredFieldInfo {
    pub fn iter_lifetime(&self) -> syn::Lifetime {
        syn::Lifetime::new("'__kirin_ir_iter_a", self.name.span())
    }

    pub fn iter_generics(&self) -> syn::Generics {
        let mut iter_generics = self.generics.clone();
        iter_generics.params.insert(
            0,
            syn::GenericParam::Lifetime(syn::LifetimeParam::new(self.iter_lifetime())),
        );
        iter_generics
    }

    pub fn filtering_function(
        &self,
        postfix: &str,
        name: &str,
        item: &str,
    ) -> proc_macro2::TokenStream {
        let name = format_ident!("{}", name);
        let item = format_ident!("{}", item);
        let iter_name = format_ident!("__{}{}", self.name, postfix);
        quote! {
            fn #name(&self) -> impl Iterator<Item = ::kirin_ir::#item> {
                #iter_name {
                    parent: self,
                    index: 0,
                }
            }
        }
    }

    pub fn filtering_iterator(&self, postfix: &str) -> proc_macro2::TokenStream {
        let name = self.name.clone();
        let iter_name = format_ident!("__{}{}", self.name, postfix);
        let iter_generics = self.iter_generics();
        let (_, ty_generics, _) = self.generics.split_for_impl();
        quote! {
            pub struct #iter_name #iter_generics {
                parent: &'__kirin_ir_iter_a #name #ty_generics,
                index: usize,
            }
        }
    }

    pub fn filtering_iterator_impl(&self, postfix: &str, item: &str) -> proc_macro2::TokenStream {
        let item = format_ident!("{}", item);
        let iter_name = format_ident!("__{}{}", self.name, postfix);
        match &self.data {
            DataInfo::Struct(info) => {
                let generated = info.struct_iter_impl(&self.name, &iter_name, &self.iter_generics(), &item);
                generated
            }
            DataInfo::Enum(variants) => {
                let iter_generics = self.iter_generics();
                let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
                    iter_generics.split_for_impl();
                let match_arms = variants
                    .iter()
                    .map(|variant_info| variant_info.variant_iterator_impl(&self.name))
                    .collect::<Vec<_>>();
                quote! {
                    #[automatically_derived]
                    impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                        type Item = ::kirin_ir::#item;
                        fn next(&mut self) -> Option<Self::Item> {
                            match self.parent {
                                #(#match_arms)*
                                _ => None,
                            }
                        }
                    }
                }
            }
        }
    }
}

pub trait FieldInfoFilter {
    fn filter_fields<F>(&self, root: &syn::DeriveInput, f: F) -> FilteredFieldInfo
    where
        F: Fn(&syn::Type) -> bool;
}

impl FieldInfoFilter for syn::Data {
    fn filter_fields<F>(&self, root: &syn::DeriveInput, f: F) -> FilteredFieldInfo
    where
        F: Fn(&syn::Type) -> bool,
    {
        match self {
            syn::Data::Struct(struct_def) => struct_def.filter_fields(root, f),
            syn::Data::Enum(enum_def) => enum_def.filter_fields(root, f),
            _ => panic!("Instruction can only be derived for structs or enums"),
        }
    }
}

impl FieldInfoFilter for syn::DataStruct {
    fn filter_fields<F>(&self, root: &syn::DeriveInput, f: F) -> FilteredFieldInfo
    where
        F: Fn(&syn::Type) -> bool,
    {
        FilteredFieldInfo {
            name: root.ident.clone(),
            generics: root.generics.clone(),
            data: DataInfo::Struct(FieldInfo::new(f, &root.ident, &self.fields)),
        }
    }
}

impl FieldInfoFilter for syn::DataEnum {
    fn filter_fields<F>(&self, root: &syn::DeriveInput, f: F) -> FilteredFieldInfo
    where
        F: Fn(&syn::Type) -> bool,
    {
        let variants = self
            .variants
            .iter()
            .map(|variant| FieldInfo::new(&f, &variant.ident, &variant.fields))
            .collect();

        FilteredFieldInfo {
            name: root.ident.clone(),
            generics: root.generics.clone(),
            data: DataInfo::Enum(variants),
        }
    }
}

impl FieldInfo {
    pub fn new<F>(f: F, name: &syn::Ident, fields: &syn::Fields) -> Self
    where
        F: Fn(&syn::Type) -> bool,
    {
        match fields {
            syn::Fields::Named(fields_named) => {
                let mut result_idents = Vec::new();
                for field in &fields_named.named {
                    if f(&field.ty) {
                        result_idents.push(field.ident.clone().unwrap());
                    }
                }
                FieldInfo::Named(name.clone(), result_idents)
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                let mut result_indices = Vec::new();
                for (i, field) in fields_unnamed.unnamed.iter().enumerate() {
                    if f(&field.ty) {
                        result_indices.push(syn::LitInt::new(&i.to_string(), field.span()));
                    }
                }
                FieldInfo::Anonymous(name.clone(), fields_unnamed.unnamed.len(), result_indices)
            }
            syn::Fields::Unit => FieldInfo::Unit,
        }
    }

    pub fn match_arms(&self) -> Vec<proc_macro2::TokenStream> {
        match self {
            FieldInfo::Anonymous(_, _, indices) => indices
                .iter()
                .enumerate()
                .map(|(i, i_lit)| {
                    let self_i = format_ident!("__self_{}", i_lit.base10_parse::<usize>().unwrap());
                    quote! {
                        #i => {
                            self.index += 1;
                            Some(#self_i.clone())
                        },
                    }
                })
                .collect(),
            FieldInfo::Named(_, idents) => idents
                .iter()
                .enumerate()
                .map(|(i, ident)| {
                    let i_lit = syn::LitInt::new(&i.to_string(), ident.span());
                    quote! {
                        #i_lit => {
                            self.index += 1;
                            Some(#ident.clone())
                        },
                    }
                })
                .collect(),
            FieldInfo::Unit => vec![],
        }
    }

    pub fn struct_iter_impl(
        &self,
        name: &syn::Ident,
        iter_name: &syn::Ident,
        iter_generics: &syn::Generics,
        item: &syn::Ident,
    ) -> proc_macro2::TokenStream {
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();
        match self {
            FieldInfo::Anonymous(_, num_fields, _) => {
                let vars = (0..*num_fields)
                    .map(|i| format_ident!("__self_{}", i))
                    .collect::<Vec<_>>();
                let arms = self.match_arms();
                quote! {
                    #[automatically_derived]
                    impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                        type Item = ::kirin_ir::#item;
                        fn next(&mut self) -> Option<Self::Item> {
                            let #name (#(#vars),*) = self.parent;
                            match self.index {
                                #(#arms)*
                                _ => None,
                            }
                        }
                    }
                }
            }
            FieldInfo::Named(_, idents) => {
                let arms = self.match_arms();
                quote! {
                    #[automatically_derived]
                    impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                        type Item = ::kirin_ir::#item;
                        fn next(&mut self) -> Option<Self::Item> {
                            let #name { #(#idents),* .. } = self.parent;
                            match self.index {
                                #(#arms)*
                                _ => None,
                            }
                        }
                    }
                }
            }
            FieldInfo::Unit => quote! {
                #[automatically_derived]
                impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = ::kirin_ir::#item;
                    fn next(&mut self) -> Option<Self::Item> {
                        None
                    }
                }
            },
        }
    }

    pub fn variant_iterator_impl(&self, name: &syn::Ident) -> proc_macro2::TokenStream {
        match self {
            FieldInfo::Anonymous(variant, num_fields, _) => {
                let vars = (0..*num_fields)
                    .map(|i| format_ident!("__self_{}", i))
                    .collect::<Vec<_>>();
                let arms = self.match_arms();
                quote! {
                    #name::#variant(#(#vars),*) => {
                        match self.index {
                            #(#arms)*
                            _ => None,
                        }
                    }
                }
            }
            FieldInfo::Named(variant, idents) => {
                let arms = self.match_arms();
                quote! {
                    #name::#variant { #(#idents),* .. } => {
                        match self.index {
                            #(#arms)*
                            _ => None,
                        }
                    }
                }
            }
            FieldInfo::Unit => quote! {},
        }
    }
}
