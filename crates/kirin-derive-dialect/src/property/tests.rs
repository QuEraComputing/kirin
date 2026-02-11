use super::{DeriveProperty, PropertyKind};
use kirin_derive_core::prelude::darling;
use kirin_test_utils::rustfmt;

fn derive_property(
    input: &syn::DeriveInput,
    kind: PropertyKind,
    trait_name: &str,
    method_name: &str,
) -> darling::Result<String> {
    let mut tokens = proc_macro2::TokenStream::new();
    let mut derive = DeriveProperty::new(kind, "::kirin::ir", trait_name, method_name, "bool");
    tokens.extend(derive.emit(input)?);
    Ok(rustfmt(tokens.to_string()))
}

fn derive_constant(input: &syn::DeriveInput) -> String {
    derive_property(input, PropertyKind::Constant, "IsConstant", "is_constant").unwrap()
}

fn derive_speculatable(input: &syn::DeriveInput) -> darling::Result<String> {
    derive_property(
        input,
        PropertyKind::Speculatable,
        "IsSpeculatable",
        "is_speculatable",
    )
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

#[test]
fn test_speculatable_requires_pure_on_struct() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(speculatable, type = TestType)]
        struct MyStruct {
            a: i32,
        }
    };

    let error = derive_speculatable(&input).expect_err("speculatable should require pure");
    assert!(
        error
            .to_string()
            .contains("effective #[kirin(speculatable)] requires #[kirin(pure)]"),
        "expected pure/speculatable invariant error, got: {}",
        error
    );
}

#[test]
fn test_speculatable_requires_pure_on_variant() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = TestType)]
        enum MyEnum {
            #[kirin(speculatable)]
            VariantA,
            VariantB,
        }
    };

    let error = derive_speculatable(&input).expect_err("speculatable should require pure");
    assert!(
        error
            .to_string()
            .contains("effectively #[kirin(speculatable)]"),
        "expected pure/speculatable invariant error, got: {}",
        error
    );
}

#[test]
fn test_speculatable_works_with_global_pure() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(pure, type = TestType)]
        enum MyEnum {
            #[kirin(speculatable)]
            VariantA,
            VariantB,
        }
    };

    let generated =
        derive_speculatable(&input).expect("global pure should allow local speculatable");
    assert!(
        generated.contains("impl ::kirin::ir::IsSpeculatable for MyEnum"),
        "expected IsSpeculatable impl generation:\n{}",
        generated
    );
    assert!(
        generated.contains("is_speculatable"),
        "expected is_speculatable method generation:\n{}",
        generated
    );
}
