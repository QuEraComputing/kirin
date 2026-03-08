use kirin::ir::{
    HasArguments, HasBlocks, HasRegions, HasResults, HasSuccessors, IsConstant, IsPure,
    IsTerminator, TestSSAValue, Typeof,
};
use kirin::pretty::{ArenaDoc, DocAllocator, Document, PrettyPrint};

use crate::Constant;

/// Minimal type for Ty parameter.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
struct TestTy;

/// Minimal value type for T parameter.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct TestVal(i64);

impl Typeof<TestTy> for TestVal {
    fn type_of(&self) -> TestTy {
        TestTy
    }
}

impl PrettyPrint for TestVal {
    fn namespaced_pretty_print<'a, L: kirin::ir::Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        doc.text(format!("{}", self.0))
    }
}

fn make_constant(val: i64) -> Constant<TestVal, TestTy> {
    Constant {
        value: TestVal(val),
        result: TestSSAValue(0).into(),
        marker: std::marker::PhantomData,
    }
}

// --- Dialect property: constant is a constant ---

#[test]
fn is_constant() {
    assert!(make_constant(0).is_constant());
    assert!(make_constant(42).is_constant());
    assert!(make_constant(-1).is_constant());
}

// --- Dialect property: constant is pure ---

#[test]
fn is_pure() {
    assert!(make_constant(0).is_pure());
}

// --- Dialect property: not a terminator ---

#[test]
fn not_terminator() {
    assert!(!make_constant(0).is_terminator());
}

// --- HasArguments: no arguments (constant has no SSA inputs) ---

#[test]
fn no_arguments() {
    let c = make_constant(42);
    assert_eq!(c.arguments().count(), 0);
}

// --- HasResults: exactly one result ---

#[test]
fn one_result() {
    let c = make_constant(42);
    assert_eq!(c.results().count(), 1);
}

// --- HasSuccessors / HasBlocks / HasRegions: all empty ---

#[test]
fn no_successors() {
    assert_eq!(make_constant(0).successors().count(), 0);
}

#[test]
fn no_blocks() {
    assert_eq!(make_constant(0).blocks().count(), 0);
}

#[test]
fn no_regions() {
    assert_eq!(make_constant(0).regions().count(), 0);
}

// --- Clone + PartialEq ---

#[test]
fn clone_eq() {
    let c = make_constant(42);
    assert_eq!(c, c.clone());
}

#[test]
fn different_values_not_equal() {
    assert_ne!(make_constant(1), make_constant(2));
}

// --- Value access ---

#[test]
fn value_field_accessible() {
    let c = make_constant(99);
    assert_eq!(c.value, TestVal(99));
}

// --- Debug formatting ---

#[test]
fn debug_contains_value() {
    let c = make_constant(42);
    let dbg = format!("{c:?}");
    assert!(
        dbg.contains("42"),
        "debug output should contain the value: {dbg}"
    );
}

// --- Typeof integration ---

#[test]
fn value_typeof() {
    let val = TestVal(10);
    assert_eq!(val.type_of(), TestTy);
}
