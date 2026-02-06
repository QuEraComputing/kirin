//! Tests for struct-based dialect definitions.

use kirin_derive::Dialect;
use kirin_ir::*;
use kirin_test_utils::*;

#[derive(Dialect, Clone, Debug, PartialEq)]
#[kirin(fn, type = SimpleIRType, crate = kirin_ir)]
struct StructOp {
    arg1: SSAValue,
    arg2: SSAValue,
    extra: String,
    res: ResultValue,
}

#[test]
fn test_struct_arguments_iterator() {
    let v1: SSAValue = TestSSAValue(1).into();
    let v2: SSAValue = TestSSAValue(2).into();

    let op = StructOp {
        arg1: v1,
        arg2: v2,
        extra: "hello".to_string(),
        res: TestSSAValue(3).into(),
    };

    let args: Vec<_> = op.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);
}

#[test]
fn test_struct_results_iterator() {
    let r1: ResultValue = TestSSAValue(3).into();

    let op = StructOp {
        arg1: TestSSAValue(1).into(),
        arg2: TestSSAValue(2).into(),
        extra: "hello".to_string(),
        res: r1,
    };

    let results: Vec<_> = op.results().cloned().collect();
    assert_eq!(results, vec![r1]);
}
