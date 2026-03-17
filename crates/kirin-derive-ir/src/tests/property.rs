//! Validation tests for bool property derives (constant/pure/speculatable interactions).

use crate::generate::*;

#[test]
fn test_is_constant_without_pure_error() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, constant)]
        struct BadConstant {
            value: i64,
        }
    };
    let result = generate_property(&input, IS_CONSTANT);
    assert!(result.is_err(), "constant without pure should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("pure"),
        "Error should mention pure requirement: {err}"
    );
}

#[test]
fn test_is_speculatable_without_pure_error() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, speculatable)]
        struct BadSpec {
            value: i64,
        }
    };
    let result = generate_property(&input, IS_SPECULATABLE);
    assert!(result.is_err(), "speculatable without pure should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("pure"),
        "Error should mention pure requirement: {err}"
    );
}

#[test]
fn test_is_constant_with_pure_succeeds() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, constant, pure)]
        struct GoodConstant {
            value: i64,
        }
    };
    let result = generate_property(&input, IS_CONSTANT);
    assert!(result.is_ok(), "constant with pure should succeed");
}

// NOTE: Design issue — BoolProperty::for_variant does not call validate(),
// so per-variant constant-without-pure is not caught at derive time.
// The validation only runs through the for_struct path (struct inputs).
// This test documents the current behavior.
#[test]
fn test_enum_variant_constant_without_pure_errors() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum Ops {
            #[kirin(constant)]
            Lit { value: i64 },
            Add { lhs: SSAValue, rhs: SSAValue },
        }
    };
    // for_variant now validates: constant requires pure
    let result = generate_property(&input, IS_CONSTANT);
    assert!(
        result.is_err(),
        "constant without pure should error on enum variants too"
    );
}
