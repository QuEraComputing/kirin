use crate::{check::trait_impl::CheckerInfo, has_attr, is_attr_option_true};

pub enum EnumChecker<'input> {
    GlobalWrapper(Vec<EnumVariantWrapper<'input>>),
    Regular(Vec<EnumVariantChecker<'input>>),
}

impl<'input> EnumChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        data: &'input syn::DataEnum,
    ) -> Self {
        if has_attr(input.attrs, "kirin", "wraps") {
            let variants = data
                .variants
                .iter()
                .map(|variant| EnumVariantWrapper::scan(checker, input, variant))
                .collect();
            Self::GlobalWrapper(variants)
        } else {
            let variants = data
                .variants
                .iter()
                .map(|variant| EnumVariantChecker::scan(checker, input, variant))
                .collect();
            Self::Regular(variants)
        }
    }
}

pub enum EnumVariantWrapper<'input> {
    Named(NamedVariantWrapper<'input>),
    Unnamed(UnnamedVariantWrapper<'input>),
}

impl<'input> EnumVariantWrapper<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        variant: &'input syn::Variant,
    ) -> Self {
        match &variant.fields {
            syn::Fields::Named(fields) => EnumVariantWrapper::Named(NamedVariantWrapper::scan(
                checker, input, variant, fields,
            )),
            syn::Fields::Unnamed(fields) => EnumVariantWrapper::Unnamed(
                UnnamedVariantWrapper::scan(checker, input, variant, fields),
            ),
            _ => panic!("wrapper variants must have named or unnamed fields"),
        }
    }
}

pub enum EnumVariantChecker<'input> {
    Named(NamedVariantChecker<'input>),
    Unnamed(UnnamedVariantChecker<'input>),
    Unit,
}

impl<'input> EnumVariantChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        variant: &'input syn::Variant,
    ) -> Self {
        match &variant.fields {
            syn::Fields::Named(fields) => EnumVariantChecker::Named(NamedVariantChecker::scan(
                checker, input, variant, fields,
            )),
            syn::Fields::Unnamed(fields) => EnumVariantChecker::Unnamed(
                UnnamedVariantChecker::scan(checker, input, variant, fields),
            ),
            syn::Fields::Unit => EnumVariantChecker::Unit,
        }
    }
}

pub enum NamedVariantChecker<'input> {
    Wrapper(NamedVariantWrapper<'input>),
    Regular(RegularVariantChecker<'input>),
}

impl<'input> NamedVariantChecker<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        variant: &'input syn::Variant,
        fields: &'input syn::FieldsNamed,
    ) -> Self {
        if has_attr(&variant.attrs, "kirin", "wraps") {
            NamedVariantChecker::Wrapper(NamedVariantWrapper::scan(checker, input, variant, fields))
        } else {
            NamedVariantChecker::Regular(RegularVariantChecker {
                checker,
                variant_name: &variant.ident,
                value: is_attr_option_true(&variant.attrs, &checker.option),
            })
        }
    }
}

pub enum UnnamedVariantChecker<'input> {
    Wrapper(RegularVariantChecker<'input>),
    Regular(UnnamedVariantWrapper<'input>),
}

pub struct RegularVariantChecker<'input> {
    checker: &'input CheckerInfo,
    variant_name: &'input syn::Ident,
    value: bool,
}

pub struct NamedVariantWrapper<'input> {
    checker: &'input CheckerInfo,
    variant_name: &'input syn::Ident,
    wraps: syn::Ident,
    wraps_type: syn::Type,
}

impl<'input> NamedVariantWrapper<'input> {
    pub fn scan(
        checker: &'input CheckerInfo,
        input: &'input syn::DeriveInput,
        variant: &'input syn::Variant,
        fields_named: &'input syn::FieldsNamed,
    ) -> Self {
        if fields_named.named.len() == 1 {
            let f = fields_named.named.first().unwrap();
            Self {
                checker,
                variant_name: &variant.ident,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            }
        } else if let Some(f) = fields_named
            .named
            .iter()
            .find(|f| has_attr(&f.attrs, "kirin", "wraps"))
        {
            Self {
                checker,
                variant_name: &variant.ident,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            }
        } else {
            panic!(
                "variant #[kirin(wraps)] attribute can only be used \
on wrapper variants with a single field,\
consider adding #[kirin(wraps)] to the specific field instead"
            );
        }
    }
}

pub struct UnnamedVariantWrapper<'input> {
    checker: &'input CheckerInfo,
    variant_name: &'input syn::Ident,
    wraps: usize,
    wraps_type: syn::Type,
}
