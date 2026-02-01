//! Tests for dialects with Vec field types.

use kirin_derive::Dialect;
use kirin_ir::*;
use kirin_test_utils::*;

// Note: 'fn' attribute is omitted to disable builder generation,
// avoiding 'ResultValue field cannot be a Vec' error.
#[derive(Dialect, Clone, Debug, PartialEq)]
#[kirin(type_lattice = SimpleTypeLattice, crate = kirin_ir)]
struct VecOp {
    args: Vec<SSAValue>,
    res: Vec<ResultValue>,
}

#[test]
fn test_vec_arguments_iterator() {
    let v1: SSAValue = TestSSAValue(1).into();
    let v2: SSAValue = TestSSAValue(2).into();

    let op = VecOp {
        args: vec![v1, v2],
        res: vec![TestSSAValue(3).into()],
    };

    let args: Vec<_> = op.arguments().cloned().collect();
    assert_eq!(args, vec![v1, v2]);
}

#[test]
fn test_vec_results_iterator() {
    let r1: ResultValue = TestSSAValue(3).into();

    let op = VecOp {
        args: vec![TestSSAValue(1).into(), TestSSAValue(2).into()],
        res: vec![r1],
    };

    let results: Vec<_> = op.results().cloned().collect();
    assert_eq!(results, vec![r1]);
}
