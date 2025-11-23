use crate::data::*;
use crate::field::*;
use crate::tests::rustfmt;

#[test]
fn test_either_enum() {
    let input: syn::DeriveInput = syn::parse_quote! {
        enum TestEnum<T> {
            VariantA { #[kirin(wraps)] wrapped: InnerStructA<T> },
            #[kirin(wraps)]
            VariantB(InnerStructB),
            VariantC { a: SSAValue, b: T, c: SSAValue },
            VariantD(SSAValue, f64, SSAValue),
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_global_enum_wrapper() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(wraps)]
        enum TestEnum<T> {
            VariantA { wrapped: InnerStructA<T> },
            VariantB(InnerStructB),
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_regular_enum() {
    let input: syn::DeriveInput = syn::parse_quote! {
        enum TestEnum<T> {
            VariantA { a: SSAValue, b: T, c: SSAValue },
            VariantB(SSAValue, f64, SSAValue),
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_arith_enum() {
    let input: syn::DeriveInput = syn::parse_quote! {
        pub enum ArithInstruction<T> {
            Add(SSAValue, Vec<SSAValue>, ResultValue, T),
            Sub(SSAValue, Vec<SSAValue>, ResultValue, T),
            Mul(SSAValue, Vec<SSAValue>, ResultValue),
            Div(SSAValue, Vec<SSAValue>, ResultValue),
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_named() {
    let input: syn::DeriveInput = syn::parse_quote! {
        pub enum ControlFlowInstruction {
            Branch {
                target: Block,
            },
            ConditionalBranch {
                condition: SSAValue,
                true_target: Block,
                false_target: Block,
            },
            Return(SSAValue),
        }
    };
    insta::assert_snapshot!(generate(input));
}

#[test]
fn test_unit_enum() {
    let input: syn::DeriveInput = syn::parse_quote! {
        enum TestEnum {
            VariantA,
            VariantB,
        }
    };
    insta::assert_snapshot!(generate(input));
}

fn generate(input: syn::DeriveInput) -> String {
    rustfmt(derive_field_iter!(
        &input,
        "arguments",
        SSAValue,
        HasArguments
    ))
}
