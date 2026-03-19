use super::semantics::{ExactSemantics, SignatureCmp, SignatureSemantics};
use super::definition::Signature;

// A simple type for testing ExactSemantics
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SimpleType {
    Int,
    Float,
    Bool,
}

#[test]
fn test_exact_applicable_match() {
    let call = Signature::new(
        vec![SimpleType::Int, SimpleType::Float],
        SimpleType::Bool,
        (),
    );
    let cand = Signature::new(
        vec![SimpleType::Int, SimpleType::Float],
        SimpleType::Bool,
        (),
    );
    assert!(ExactSemantics::applicable(&call, &cand).is_some());
}

#[test]
fn test_exact_applicable_mismatch() {
    let call = Signature::new(vec![SimpleType::Int], SimpleType::Bool, ());
    let cand = Signature::new(vec![SimpleType::Float], SimpleType::Bool, ());
    assert!(ExactSemantics::applicable(&call, &cand).is_none());
}

#[test]
fn test_exact_cmp_equal() {
    let a = Signature::new(vec![SimpleType::Int], SimpleType::Bool, ());
    let b = a.clone();
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Equal
    );
}

#[test]
fn test_exact_cmp_incomparable() {
    let a = Signature::new(vec![SimpleType::Int], SimpleType::Bool, ());
    let b = Signature::new(vec![SimpleType::Float], SimpleType::Bool, ());
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Incomparable
    );
}
