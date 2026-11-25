use super::definition::Signature;
use super::semantics::{ExactSemantics, SignatureCmp, SignatureSemantics};

// A simple type for testing ExactSemantics
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SimpleType {
    Int,
    Float,
    Bool,
}

impl std::fmt::Display for SimpleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimpleType::Int => write!(f, "Int"),
            SimpleType::Float => write!(f, "Float"),
            SimpleType::Bool => write!(f, "Bool"),
        }
    }
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

#[test]
fn test_exact_applicable_requires_matching_rets() {
    let call = Signature::with_rets(
        vec![SimpleType::Int],
        vec![SimpleType::Bool, SimpleType::Int],
        (),
    );
    let cand = Signature::new(vec![SimpleType::Int], SimpleType::Bool, ());
    assert!(ExactSemantics::applicable(&call, &cand).is_none());
    assert!(ExactSemantics::applicable(&call, &call).is_some());
}

#[test]
fn test_rets_to_string_wraps_none_typed_single_return() {
    let sig = Signature::new(vec![], "None", ());
    assert_eq!(sig.rets_to_string(), "(None)");
}
