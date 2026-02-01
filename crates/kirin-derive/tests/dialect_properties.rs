//! Tests for dialect property attributes (pure, terminator, constant).

use kirin_derive::Dialect;
use kirin_ir::*;
use kirin_test_utils::*;

#[derive(Dialect, Clone, Debug, PartialEq)]
#[kirin(fn, type_lattice = SimpleTypeLattice, crate = kirin_ir)]
enum PropertyLang {
    #[kirin(pure)]
    Add(SSAValue, SSAValue, ResultValue),
    #[kirin(terminator)]
    Return(SSAValue),
    #[kirin(constant)]
    Const(i64, ResultValue),
    Other(SSAValue),
}

#[test]
fn test_pure_property() {
    let add = PropertyLang::Add(
        TestSSAValue(1).into(),
        TestSSAValue(2).into(),
        TestSSAValue(3).into(),
    );
    assert!(add.is_pure());
    assert!(!add.is_terminator());
    assert!(!add.is_constant());
}

#[test]
fn test_terminator_property() {
    let ret = PropertyLang::Return(TestSSAValue(1).into());
    assert!(!ret.is_pure());
    assert!(ret.is_terminator());
    assert!(!ret.is_constant());
}

#[test]
fn test_constant_property() {
    let c = PropertyLang::Const(42, TestSSAValue(3).into());
    assert!(!c.is_pure());
    assert!(!c.is_terminator());
    assert!(c.is_constant());
}

#[test]
fn test_no_properties() {
    let other = PropertyLang::Other(TestSSAValue(1).into());
    assert!(!other.is_pure());
    assert!(!other.is_terminator());
    assert!(!other.is_constant());
}
