mod context;
mod enum_impl;
mod struct_impl;

use crate::data::{Alt, Emit};
use enum_impl::EnumImpl;
use struct_impl::StructImpl;

pub type PropertyImpl = Alt<StructImpl, EnumImpl>;
pub use context::{IsConstant, IsPure, IsTerminator, Property, SearchProperty};

impl<S: SearchProperty> Emit<'_> for Property<S> {
    type Output = PropertyImpl;
}

#[cfg(test)]
mod tests {
    use crate::data::*;
    use super::*;

    #[test]
    fn test_struct_regular() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(constant, type_lattice = Lattice)]
            struct MyStruct {
                a: i32,
                b: i32,
            }
        };

        let content = Property::<IsConstant>::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("IsConstant")
            .trait_method("is_constant")
            .value_type("bool")
            .build()
            .print(&input);
        insta::assert_snapshot!(content);
    }

    #[test]
    fn test_struct_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = Lattice)]
            struct Wrapper<T> {
                #[wraps]
                inner: InnerStruct<T>,
            }
        };

        let content = Property::<IsConstant>::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("IsConstant")
            .trait_method("is_constant")
            .value_type("bool")
            .build()
            .print(&input);
        insta::assert_snapshot!(content);
    }

    #[test]
    fn test_enum_regular() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = Lattice)]
            enum MyEnum<T> {
                VariantA { a: i32, b: T },
                #[kirin(constant)]
                VariantB(i32, T),
            }
        };

        let content = Property::<IsConstant>::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("IsConstant")
            .trait_method("is_constant")
            .value_type("bool")
            .build()
            .print(&input);
        insta::assert_snapshot!(content);
    }

    #[test]
    fn test_enum_wrapper() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = Lattice, constant)]
            #[wraps]
            enum MyEnum<T> {
                VariantA { inner: InnerStructA<T> },
                VariantB(InnerStructB),
            }
        };

        let content = Property::<IsConstant>::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("IsConstant")
            .trait_method("is_constant")
            .value_type("bool")
            .build()
            .print(&input);
        insta::assert_snapshot!(content);
    }

    #[test]
    fn test_enum_mixed() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = Lattice)]
            enum MyEnum<T> {
                VariantA { #[wraps] inner: InnerStructA<T> },
                #[wraps]
                VariantB(InnerStructB),
                VariantC { a: i32, b: T },
                #[kirin(constant)]
                VariantD(i32, T),
            }
        };

        let content = Property::<IsConstant>::builder()
            .default_crate_path("::kirin::ir")
            .trait_path("IsConstant")
            .trait_method("is_constant")
            .value_type("bool")
            .build()
            .print(&input);
        insta::assert_snapshot!(content);
    }
}
