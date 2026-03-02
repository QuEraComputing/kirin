use super::semantics::{ExactSemantics, SignatureCmp, SignatureSemantics};
use super::signature::Signature;

// A simple type for testing ExactSemantics
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SimpleType {
    Int,
    Float,
    Bool,
}

#[test]
fn test_exact_applicable_match() {
    let call = Signature {
        params: vec![SimpleType::Int, SimpleType::Float],
        ret: SimpleType::Bool,
        constraints: (),
    };
    let cand = Signature {
        params: vec![SimpleType::Int, SimpleType::Float],
        ret: SimpleType::Bool,
        constraints: (),
    };
    assert!(ExactSemantics::applicable(&call, &cand).is_some());
}

#[test]
fn test_exact_applicable_mismatch() {
    let call = Signature {
        params: vec![SimpleType::Int],
        ret: SimpleType::Bool,
        constraints: (),
    };
    let cand = Signature {
        params: vec![SimpleType::Float],
        ret: SimpleType::Bool,
        constraints: (),
    };
    assert!(ExactSemantics::applicable(&call, &cand).is_none());
}

#[test]
fn test_exact_cmp_equal() {
    let a = Signature {
        params: vec![SimpleType::Int],
        ret: SimpleType::Bool,
        constraints: (),
    };
    let b = a.clone();
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Equal
    );
}

#[test]
fn test_exact_cmp_incomparable() {
    let a = Signature {
        params: vec![SimpleType::Int],
        ret: SimpleType::Bool,
        constraints: (),
    };
    let b = Signature {
        params: vec![SimpleType::Float],
        ret: SimpleType::Bool,
        constraints: (),
    };
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Incomparable
    );
}
