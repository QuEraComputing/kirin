mod named_regular;
mod named_wrapper;
mod unnamed_regular;
mod unnamed_wrapper;

use {
    crate::field::data::{AccessorInfo, has_attr},
    named_regular::NamedStructRegularAccessor,
    named_wrapper::NamedStructWrapperAccessor,
    proc_macro2::TokenStream,
    unnamed_regular::UnnamedStructRegularAccessor,
    unnamed_wrapper::UnnamedStructWrapperAccessor,
};

pub enum StructAccessor<'input> {
    Named(NamedStructAccessor<'input>),
    Unnamed(UnnamedStructAccessor<'input>),
}

impl<'input> StructAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, data: &'input syn::DataStruct) -> Self {
        match &data.fields {
            syn::Fields::Named(fields) => {
                StructAccessor::Named(NamedStructAccessor::scan(info, input, fields))
            }
            syn::Fields::Unnamed(fields) => {
                StructAccessor::Unnamed(UnnamedStructAccessor::scan(info, input, fields))
            }
            _ => panic!("only named and unnamed fields are supported"),
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            StructAccessor::Named(named) => named.generate(),
            StructAccessor::Unnamed(unnamed) => unnamed.generate(),
        }
    }
}

pub enum NamedStructAccessor<'input> {
    Wrapper(NamedStructWrapperAccessor<'input>),
    Regular(NamedStructRegularAccessor<'input>),
}

impl<'input> NamedStructAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, fields: &'input syn::FieldsNamed) -> Self {
        if has_attr(&input.attrs, "kirin", "wraps") {
            if fields.named.len() != 1 {
                panic!(
                    "global #[kirin(wraps)] attribute can only be used \
on wrapper structs with a single field,\
consider adding #[kirin(wraps)] to the specific field instead"
                );
            }
            let f = fields.named.first().unwrap();
            NamedStructAccessor::Wrapper(NamedStructWrapperAccessor {
                info,
                name: &input.ident,
                generics: &input.generics,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else if let Some(f) = fields
            .named
            .iter()
            .find(|f| has_attr(&f.attrs, "kirin", "wraps"))
        {
            NamedStructAccessor::Wrapper(NamedStructWrapperAccessor {
                info,
                name: &input.ident,
                generics: &input.generics,
                wraps: f.ident.clone().unwrap(),
                wraps_type: f.ty.clone(),
            })
        } else {
            NamedStructAccessor::Regular(NamedStructRegularAccessor::scan(info, input, fields))
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            NamedStructAccessor::Wrapper(wrapper) => wrapper.generate(),
            NamedStructAccessor::Regular(regular) => regular.generate(),
        }
    }
}

pub enum UnnamedStructAccessor<'input> {
    Wrapper(UnnamedStructWrapperAccessor<'input>),
    Regular(UnnamedStructRegularAccessor<'input>),
}

impl<'input> UnnamedStructAccessor<'input> {
    pub fn scan(info: &'input AccessorInfo, input: &'input syn::DeriveInput, fields: &'input syn::FieldsUnnamed) -> Self {
        if has_attr(&input.attrs, "kirin", "wraps") {
            if fields.unnamed.len() != 1 {
                panic!(
                    "global #[kirin(wraps)] attribute can only be used \
on wrapper structs with a single field,\
consider adding #[kirin(wraps)] to the specific field instead"
                );
            }
            UnnamedStructAccessor::Wrapper(UnnamedStructWrapperAccessor {
                info,
                name: &input.ident,
                generics: &input.generics,
                wraps: 0,
                wraps_type: fields.unnamed.first().unwrap().ty.clone(),
            })
        } else if let Some((index, f)) = fields
            .unnamed
            .iter()
            .enumerate()
            .find(|(_, f)| has_attr(&f.attrs, "kirin", "wraps"))
        {
            UnnamedStructAccessor::Wrapper(UnnamedStructWrapperAccessor {
                info,
                name: &input.ident,
                generics: &input.generics,
                wraps: index,
                wraps_type: f.ty.clone(),
            })
        } else {
            UnnamedStructAccessor::Regular(UnnamedStructRegularAccessor::scan(info, input, fields))
        }
    }

    pub fn generate(&self) -> TokenStream {
        match self {
            UnnamedStructAccessor::Wrapper(wrapper) => wrapper.generate(),
            UnnamedStructAccessor::Regular(regular) => regular.generate(),
        }
    }
}
