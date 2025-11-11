mod global_wrapper;
mod regular;
mod some_wrapper;
mod variant;

use {
    crate::field::data::has_attr, global_wrapper::EnumGlobalWrapperAccessor, regular::EnumRegularAccessor, some_wrapper::EnumSomeWrapperAccessor
};

pub enum EnumAccessor<'input> {
    GlobalWrapper(EnumGlobalWrapperAccessor<'input>),
    SomeWrapper(EnumSomeWrapperAccessor<'input>),
    Regular(EnumRegularAccessor<'input>),
}

impl<'input> EnumAccessor<'input> {
    pub fn scan(
        info: &'input crate::field::data::AccessorInfo,
        input: &'input syn::DeriveInput,
        data: &'input syn::DataEnum,
    ) -> Self {
        if has_attr(&input.attrs, "kirin", "wraps") {
            EnumAccessor::GlobalWrapper(EnumGlobalWrapperAccessor::scan(info, input, data))
        } else if data.variants.iter().any(|v| {
            has_attr(&v.attrs, "kirin", "wraps")
        }) {
            EnumAccessor::SomeWrapper(EnumSomeWrapperAccessor::scan(info, input, data))
        } else {
            EnumAccessor::Regular(EnumRegularAccessor::scan(info, input, data))
        }
    }

    pub fn generate(&self) -> proc_macro2::TokenStream {
        match self {
            EnumAccessor::GlobalWrapper(g) => g.generate(),
            EnumAccessor::SomeWrapper(s) => s.generate(),
            EnumAccessor::Regular(r) => r.generate(),
        }
    }
}
