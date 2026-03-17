//! Snapshot tests for standalone single-trait derive macros
//! (HasArguments, HasResults, HasRegions, HasDigraphs, HasUngraphs, IsTerminator, IsEdge).

use crate::generate::*;
use kirin_test_utils::rustfmt;

#[test]
fn test_standalone_is_terminator() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, terminator)]
        struct Return {
            value: Value,
        }
    };
    let tokens =
        generate_property(&input, IS_TERMINATOR).expect("Failed to generate IsTerminator");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_has_arguments() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct BinaryOp {
            result: SSAValue,
            lhs: Value,
            rhs: Value,
        }
    };
    let tokens =
        generate_field_iter(&input, HAS_ARGUMENTS).expect("Failed to generate HasArguments");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_has_results() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct UnaryOp {
            #[kirin(type = SimpleType::placeholder())]
            result: ResultValue,
            arg: SSAValue,
        }
    };
    let tokens =
        generate_field_iter(&input, HAS_RESULTS).expect("Failed to generate HasResults");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_has_regions() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct Lambda {
            body: Region,
        }
    };
    let tokens =
        generate_field_iter(&input, HAS_REGIONS).expect("Failed to generate HasRegions");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_has_digraphs() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct QuantumEval {
            body: DiGraph,
        }
    };
    let tokens =
        generate_field_iter(&input, HAS_DIGRAPHS).expect("Failed to generate HasDigraphs");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_has_ungraphs() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType)]
        struct ZxEval {
            body: UnGraph,
        }
    };
    let tokens =
        generate_field_iter(&input, HAS_UNGRAPHS).expect("Failed to generate HasUngraphs");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}

#[test]
fn test_standalone_is_edge() {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[kirin(type = SimpleType, edge)]
        struct Wire {
            #[kirin(type = SimpleType::placeholder())]
            res: ResultValue,
        }
    };
    let tokens = generate_property(&input, IS_EDGE).expect("Failed to generate IsEdge");
    insta::assert_snapshot!(rustfmt(tokens.to_string()));
}
