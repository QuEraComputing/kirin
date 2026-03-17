//! Snapshot and error tests for `#[derive(StageMeta)]` codegen.

use kirin_test_utils::rustfmt;

fn generate_stage_meta_code(input: syn::DeriveInput) -> String {
    let tokens = kirin_derive_toolkit::stage_info::generate(&input)
        .expect("Failed to generate StageMeta derive");
    rustfmt(tokens.to_string())
}

#[test]
fn test_stage_meta_single_dialect() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[stage(crate = "kirin_ir")]
        enum SimpleStage {
            #[stage(name = "arith")]
            Arith(StageInfo<ArithDialect>),
        }
    };
    insta::assert_snapshot!(generate_stage_meta_code(input));
}

#[test]
fn test_stage_meta_multi_dialect() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[stage(crate = "kirin_ir")]
        enum CompositeStage {
            #[stage(name = "arith")]
            Arith(StageInfo<ArithDialect>),
            #[stage(name = "func")]
            Func(StageInfo<FuncDialect>),
            #[stage(name = "cf")]
            Cf(StageInfo<CfDialect>),
        }
    };
    insta::assert_snapshot!(generate_stage_meta_code(input));
}

#[test]
fn test_stage_meta_duplicate_dialect() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[stage(crate = "kirin_ir")]
        enum MultiArithStage {
            #[stage(name = "arith_opt")]
            ArithOpt(StageInfo<ArithDialect>),
            #[stage(name = "arith_lower")]
            ArithLower(StageInfo<ArithDialect>),
        }
    };
    insta::assert_snapshot!(generate_stage_meta_code(input));
}

#[test]
fn test_stage_meta_on_struct_error() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[stage(crate = "kirin_ir")]
        struct NotAnEnum {
            info: StageInfo<ArithDialect>,
        }
    };
    let result = kirin_derive_toolkit::stage_info::generate(&input);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("enum"), "Expected enum-only error: {err}");
}

#[test]
fn test_stage_meta_empty_enum_error() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[stage(crate = "kirin_ir")]
        enum EmptyStage {}
    };
    let result = kirin_derive_toolkit::stage_info::generate(&input);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least one"),
        "Expected at-least-one error: {err}"
    );
}
