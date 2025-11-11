use proc_macro2::TokenStream;

use crate::{check::trait_impl::CheckerInfo, has_attr, is_attr_option_true};

pub enum StructChecker<'input> {
    Named(NamedStructChecker<'input>),
    Unnamed(UnnamedStructChecker<'input>),
}

impl<'input> StructChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        data: &'input syn::DataStruct,
    ) -> Self {
        match &data.fields {
            syn::Fields::Named(fields) => {
                StructChecker::Named(NamedStructChecker::scan(checker, input, fields))
            }
            syn::Fields::Unnamed(fields) => {
                StructChecker::Unnamed(UnnamedStructChecker::scan(checker, input, fields))
            }
            _ => panic!("only named and unnamed fields are supported"),
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            StructChecker::Named(named) => named.generate(),
            StructChecker::Unnamed(unnamed) => unnamed.generate(),
        }
    }
}

pub enum NamedStructChecker<'input> {
    Wrapper(NamedStructWrapperChecker<'input>),
    Regular(StructRegularChecker<'input>),
}

impl<'input> NamedStructChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        fields: &'input syn::FieldsNamed,
    ) -> Self {
        if has_attr(&input.attrs, "kirin", "wraps") {
            if fields.named.len() != 1 {
                panic!(
                    "global #[kirin(wraps)] attribute can only be used \
on wrapper structs with a single field,\
consider adding #[kirin(wraps)] to the specific field instead"
                );
            }
            let wraps = fields.named.first().unwrap().ident.clone().unwrap();
            let wraps_type = fields.named.first().unwrap().ty.clone();
            NamedStructChecker::Wrapper(NamedStructWrapperChecker {
                checker,
                name: &input.ident,
                generics: &input.generics,
                wraps,
                wraps_type,
            })
        } else {
            NamedStructChecker::Regular(StructRegularChecker {
                checker,
                name: &input.ident,
                generics: &input.generics,
                value: is_attr_option_true(&input.attrs, &checker.option),
            })
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            NamedStructChecker::Wrapper(wrapper) => wrapper.generate(),
            NamedStructChecker::Regular(regular) => regular.generate(),
        }
    }
}

pub enum UnnamedStructChecker<'input> {
    Wrapper(UnnamedStructWrapperChecker<'input>),
    Regular(StructRegularChecker<'input>),
}

impl<'input> UnnamedStructChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        fields: &'input syn::FieldsUnnamed,
    ) -> Self {
        if has_attr(&input.attrs, "kirin", "wraps") {
            if fields.unnamed.len() != 1 {
                panic!(
                    "global #[kirin(wraps)] attribute can only be used \
on wrapper structs with a single field,\
consider adding #[kirin(wraps)] to the specific field instead"
                );
            }
            let wraps = 0;
            let wraps_type = fields.unnamed.first().unwrap().ty.clone();
            UnnamedStructChecker::Wrapper(UnnamedStructWrapperChecker {
                checker,
                name: &input.ident,
                generics: &input.generics,
                wraps,
                wraps_type,
            })
        } else {
            UnnamedStructChecker::Regular(StructRegularChecker {
                checker,
                name: &input.ident,
                generics: &input.generics,
                value: is_attr_option_true(&input.attrs, &checker.option),
            })
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            UnnamedStructChecker::Wrapper(wrapper) => wrapper.generate(),
            UnnamedStructChecker::Regular(regular) => regular.generate(),
        }
    }
}

pub struct StructRegularChecker<'input> {
    checker: &'input CheckerInfo,
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    value: bool,
}

impl StructRegularChecker<'_> {
    pub fn generate(&self) -> TokenStream {
        let value = self.value;
        let name = &self.name;
        let checker_name = &self.checker.name;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let trait_path = &self.checker.trait_path;
        quote::quote! {
            impl #impl_generics #trait_path for #name #ty_generics #where_clause {
                fn #checker_name(&self) -> bool {
                    #value
                }
            }
        }
    }
}

pub struct NamedStructWrapperChecker<'input> {
    checker: &'input CheckerInfo,
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    wraps: syn::Ident,
    wraps_type: syn::Type,
}

impl NamedStructWrapperChecker<'_> {
    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let wraps = &self.wraps;
        let wraps_type = &self.wraps_type;
        let checker_name = &self.checker.name;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let trait_path = &self.checker.trait_path;

        quote::quote! {
            impl #impl_generics #trait_path for #name #ty_generics #where_clause {
                fn #checker_name(&self) -> bool {
                    let Self { #wraps, .. } = self;
                    <#wraps_type as #trait_path>::#checker_name(#wraps)
                }
            }
        }
    }
}

pub struct UnnamedStructWrapperChecker<'input> {
    checker: &'input CheckerInfo,
    name: &'input syn::Ident,
    generics: &'input syn::Generics,
    wraps: usize,
    wraps_type: syn::Type,
}

impl UnnamedStructWrapperChecker<'_> {
    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        let wraps = self.wraps;
        let vars = (0..=wraps)
            .map(|i| syn::Ident::new(&format!("field{}", i), proc_macro2::Span::call_site()))
            .collect::<Vec<_>>();
        let wrap_name = vars.last().unwrap();
        let wraps_type = &self.wraps_type;
        let checker_name = &self.checker.name;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let trait_path = &self.checker.trait_path;

        quote::quote! {
            impl #impl_generics #trait_path for #name #ty_generics #where_clause {
                fn #checker_name(&self) -> bool {
                    let Self ( #(#vars,)*,.. ) = self;
                    <#wraps_type as #trait_path>::#checker_name(&#wrap_name)
                }
            }
        }
    }
}
