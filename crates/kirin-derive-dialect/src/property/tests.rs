use super::{DeriveProperty, PropertyKind};
use kirin_test_utils::rustfmt;

fn derive_constant(input: &syn::DeriveInput) -> String {
    let mut tokens = proc_macro2::TokenStream::new();
    let mut derive = DeriveProperty::new(
        PropertyKind::Constant,
        "::kirin::ir",
        "IsConstant",
        "is_constant",
        "bool",
    );
    tokens.extend(derive.emit(input).unwrap());
    rustfmt(tokens.to_string())
}

macro_rules! case {
    ($($tt:tt)*) => {{
        let input: syn::DeriveInput = syn::parse_quote! {
            $($tt)*
        };
        derive_constant(&input)
    }};
}

#[test]
fn test_struct_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(constant, type = TestType)]
        struct MyStruct {
            a: i32,
            b: i32,
        }
    });
}

#[test]
fn test_struct_uses_crate_trait_path() {
    let generated = case! {
        #[kirin(constant, type = TestType)]
        struct MyStruct {
            a: i32,
        }
    };
    assert!(
        generated.contains("impl ::kirin::ir::IsConstant for MyStruct"),
        "struct property impls must use crate-qualified trait paths:\n{}",
        generated
    );
}

#[test]
fn test_struct_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        struct Wrapper<T> {
            #[wraps]
            inner: InnerStruct<T>,
        }
    });
}

#[test]
fn test_enum_regular() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        enum MyEnum<T> {
            VariantA { a: i32, b: T },
            #[kirin(constant)]
            VariantB(i32, T),
        }
    });
}

#[test]
fn test_enum_wrapper() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType, constant)]
        #[wraps]
        enum MyEnum<T> {
            VariantA { inner: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    });
}

#[test]
fn test_enum_wrapper_uses_crate_trait_path() {
    let generated = case! {
        #[kirin(type = TestType, constant)]
        #[wraps]
        enum MyEnum<T> {
            VariantA { inner: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    };
    assert!(
        generated.contains("as ::kirin::ir::IsConstant"),
        "wrapper variant calls must use crate-qualified trait path:\n{}",
        generated
    );
}

#[test]
fn test_enum_mixed() {
    insta::assert_snapshot!(case! {
        #[kirin(type = TestType)]
        enum MyEnum<T> {
            VariantA { #[wraps] inner: InnerStructA<T> },
            #[wraps]
            VariantB(InnerStructB),
            VariantC { a: i32, b: T },
            #[kirin(constant)]
            VariantD(i32, T),
        }
    });
}
