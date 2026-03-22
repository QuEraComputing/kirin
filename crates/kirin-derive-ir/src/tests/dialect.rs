//! Snapshot tests for the full `#[derive(Dialect)]` codegen.

use crate::generate::*;
use kirin_test_utils::rustfmt;

fn generate_dialect_code(input: syn::DeriveInput) -> String {
    let tokens = generate_dialect(&input).expect("Failed to generate Dialect derive");
    rustfmt(tokens.to_string())
}

#[test]
fn test_dialect_derive_struct_with_ssa_fields() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct BinaryOp {
            result: SSAValue,
            lhs: Value,
            rhs: Value,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_with_region_block() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct IfOp {
            condition: Value,
            then_block: Block,
            else_block: Block,
            body: Region,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_with_successors() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct Branch {
            target: Successor,
            args: Value,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_terminator() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, terminator)]
        struct Return {
            value: Value,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_all_properties() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, constant, pure, speculatable)]
        struct Constant {
            #[kirin(type = SimpleType::placeholder())]
            result: ResultValue,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_enum_with_wraps() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum ArithLanguage {
            #[wraps]
            Add(AddOp),
            #[wraps]
            Sub(SubOp),
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_enum_wraps_with_terminator() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum CfOps {
            #[wraps]
            Branch(BranchOp),
            #[wraps]
            #[kirin(terminator)]
            Return(ReturnOp),
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_custom_crate_path() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, crate = kirin_ir)]
        struct Nop {}
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_union_error() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        union MyUnion {
            x: i32,
            y: f32,
        }
    };
    let result = generate_dialect(&input);
    assert!(result.is_err(), "union should produce an error");
}

#[test]
fn test_dialect_derive_struct_no_fields() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct Nop {}
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_vec_ssa_value() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct CallOp {
            args: Vec<SSAValue>,
            #[kirin(type = SimpleType::placeholder())]
            result: ResultValue,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_option_block() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct ConditionalOp {
            cond: SSAValue,
            then_block: Block,
            else_block: Option<Block>,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_symbol() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct CallExtern {
            target: Symbol,
            args: Vec<SSAValue>,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_enum_mixed_wraps_and_fields() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        enum MixedOps {
            #[wraps]
            Add(AddOp),
            Literal { value: i64 },
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_with_digraph() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct QuantumEval {
            qubit: SSAValue,
            angle: SSAValue,
            body: DiGraph,
            #[kirin(type = SimpleType::placeholder())]
            res: ResultValue,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_with_ungraph() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct ZxEval {
            boundary: Vec<SSAValue>,
            captures: Vec<SSAValue>,
            body: UnGraph,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

#[test]
fn test_dialect_derive_struct_edge() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, edge)]
        struct ZxWire {
            #[kirin(type = SimpleType::placeholder())]
            res: ResultValue,
        }
    };
    insta::assert_snapshot!(generate_dialect_code(input));
}

/// Regression test for P0-1: wrapper struct HasSignature codegen must destructure
/// `self` before referencing the wrapper binding (e.g., `field_0`).
#[test]
fn test_dialect_derive_wrapper_struct_has_signature() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        #[wraps]
        struct WrapperOp(InnerOp);
    };
    let code = generate_dialect_code(input);
    // The generated HasSignature impl must contain `let Self ... = self;`
    // to destructure self before using the wrapper binding.
    assert!(
        code.contains("let Self"),
        "P0-1 regression: wrapper struct HasSignature must destructure self.\n\
         Generated code:\n{code}"
    );
    insta::assert_snapshot!(code);
}

/// Regression test for P1-10: From impl for wrapper variant with extra side-fields
/// must include the wrapped value in the constructor, not silently discard it.
#[test]
fn test_dialect_derive_enum_wraps_with_extra_fields_from_impl() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, builders)]
        enum MixedWraps {
            #[wraps]
            Simple(SimpleOp),
            Wrapped {
                #[wraps]
                inner: InnerOp,
                tag: i64,
            },
        }
    };
    let code = generate_dialect_code(input);
    // The From<InnerOp> impl for the Wrapped variant must use `value`
    // in the constructor, not discard it.
    assert!(
        code.contains("inner: value") || code.contains("inner : value"),
        "P1-10 regression: From impl for wrapper variant with extra fields \
         must include the wrapped value.\nGenerated code:\n{code}"
    );
    insta::assert_snapshot!(code);
}
