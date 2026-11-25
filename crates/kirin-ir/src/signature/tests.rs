use super::definition::Signature;
use super::semantics::{ExactSemantics, SignatureCmp, SignatureSemantics};

#[test]
fn chained_parameters_and_returns_append_in_order() {
    let signature = Signature::new()
        .takes(["A"])
        .returns("A")
        .takes(["B", "C"])
        .returns_each(["B", "C"])
        .takes([])
        .returns_each([]);
    assert_eq!(signature.params().as_slice(), &["A", "B", "C"]);
    assert_eq!(signature.rets().as_slice(), &["A", "B", "C"]);
}

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
    let call = Signature::new()
        .takes([SimpleType::Int, SimpleType::Float])
        .returns(SimpleType::Bool);
    let cand = Signature::new()
        .takes([SimpleType::Int, SimpleType::Float])
        .returns(SimpleType::Bool);
    assert!(ExactSemantics::applicable(&call, &cand).is_some());
}

#[test]
fn test_exact_applicable_mismatch() {
    let call = Signature::new()
        .takes([SimpleType::Int])
        .returns(SimpleType::Bool);
    let cand = Signature::new()
        .takes([SimpleType::Float])
        .returns(SimpleType::Bool);
    assert!(ExactSemantics::applicable(&call, &cand).is_none());
}

#[test]
fn test_exact_cmp_equal() {
    let a = Signature::new()
        .takes([SimpleType::Int])
        .returns(SimpleType::Bool);
    let b = a.clone();
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Equal
    );
}

#[test]
fn test_exact_cmp_incomparable() {
    let a = Signature::new()
        .takes([SimpleType::Int])
        .returns(SimpleType::Bool);
    let b = Signature::new()
        .takes([SimpleType::Float])
        .returns(SimpleType::Bool);
    assert_eq!(
        ExactSemantics::cmp_candidate(&a, &(), &b, &()),
        SignatureCmp::Incomparable
    );
}

#[test]
fn test_exact_applicable_requires_matching_rets() {
    let call = Signature::new()
        .takes([SimpleType::Int])
        .returns_each([SimpleType::Bool, SimpleType::Int]);
    let cand = Signature::new()
        .takes([SimpleType::Int])
        .returns(SimpleType::Bool);
    assert!(ExactSemantics::applicable(&call, &cand).is_none());
    assert!(ExactSemantics::applicable(&call, &call).is_some());
}

#[test]
fn test_display_keeps_parenthesized_types_as_one_return() {
    // A type that prints with parentheses is still a single return: the
    // return list itself is never parenthesized.
    assert_eq!(Signature::new().returns("()").to_string(), "() -> ()");
    assert_eq!(
        Signature::new().returns("(A, B)").to_string(),
        "() -> (A, B)"
    );
    assert_eq!(
        Signature::new().returns_each(["A", "B"]).to_string(),
        "() -> A, B"
    );
    assert_eq!(
        Signature::new()
            .takes(["A"])
            .returns_each(Vec::<&str>::new())
            .to_string(),
        "(A)"
    );
}
