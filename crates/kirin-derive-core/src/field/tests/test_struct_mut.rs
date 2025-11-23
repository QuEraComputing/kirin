use crate::data::*;
use crate::field::*;
use crate::tests::rustfmt;

#[test]
fn test_regular() {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct<T> {
            a: SSAValue,
            b: f64,
            c: T,
        }
    };
    insta::assert_snapshot!(generate(input));

    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct<T> {
            a: SSAValue,
            b: SSAValue,
            c: T,
        }
    };
    insta::assert_snapshot!(generate(input));

    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct<T> {
            a: SSAValue,
            b: SSAValue,
            c: Vec<SSAValue>,
            d: T,
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_named_struct_wrapper() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(wraps)]
        struct TestStruct<T> {
            wrapped: InnerStruct<T>,
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_unnamed_struct_wrapper() {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct<T>(SSAValue, #[kirin(wraps)] T, SSAValue, String, f64);
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_unnamed_struct_regular() {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct(SSAValue, SSAValue, SSAValue);
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_unit_struct() {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct TestStruct;
    };
    insta::assert_snapshot!(generate(input));
}

fn generate(input: syn::DeriveInput) -> String {
    rustfmt(derive_field_iter_mut!(
        &input,
        "arguments_mut",
        SSAValue,
        HasArgumentsMut
    ))
}
