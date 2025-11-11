mod data;
mod enum_impl;
mod struct_impl;

pub use data::AccessorInfo;
use enum_impl::EnumAccessor;
use struct_impl::StructAccessor;
pub enum DataAccessor<'input> {
    Struct(StructAccessor<'input>),
    Enum(EnumAccessor<'input>),
}

impl<'input> DataAccessor<'input> {
    pub fn scan(info: &'input data::AccessorInfo, input: &'input syn::DeriveInput) -> Self {
        match &input.data {
            syn::Data::Struct(data) => {
                DataAccessor::Struct(StructAccessor::scan(info, input, data))
            }
            syn::Data::Enum(data) => DataAccessor::Enum(EnumAccessor::scan(info, input, data)),
            _ => panic!("only structs and enums are supported"),
        }
    }

    pub fn generate(&self) -> proc_macro2::TokenStream {
        match self {
            DataAccessor::Struct(s) => s.generate(),
            DataAccessor::Enum(e) => e.generate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::rustfmt;

    #[test]
    fn test_data_accessor_struct() {
        let info = AccessorInfo::new("arguments", "SSAValue", "::kirin_ir::HasArguments");
        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStruct<T> {
                a: SSAValue,
                b: f64,
                c: T,
            }
        };
        let accessor = DataAccessor::scan(&info, &input);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated));

        let input: syn::DeriveInput = syn::parse_quote! {
            struct TestStructNamedWrap<T> {
                #[kirin(wraps)]
                a: Other,
                b: f64,
                c: T,
            }
        };
        let accessor = DataAccessor::scan(&info, &input);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated));

        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { a: SSAValue, b: T, c: SSAValue },
                VariantB(SSAValue, f64, SSAValue),
            }
        };
        let accessor = DataAccessor::scan(&info, &input);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated));

        let input: syn::DeriveInput = syn::parse_quote! {
            enum TestEnum<T> {
                VariantA { a: SSAValue, b: T, c: SSAValue },
                VariantB(SSAValue, f64, SSAValue),
            }
        };
        let accessor = DataAccessor::scan(&info, &input);
        let generated = accessor.generate();
        insta::assert_snapshot!(rustfmt(generated));
    }
}
