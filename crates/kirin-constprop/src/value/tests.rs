use kirin_arith::{CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{BranchCondition, HasProductValue};
use kirin_ir::{Lattice, Product};
use kirin_test_utils::lattice::assert_finite_lattice_laws;

use super::ConstPropValue;

type Value = ConstPropValue<i64, &'static str, &'static str>;

#[test]
fn scalar_constants_join_to_top_when_they_disagree() {
    assert_eq!(Value::Const(7).join(&Value::Const(7)), Value::Const(7));
    assert_eq!(Value::Const(7).join(&Value::Const(9)), Value::Top);
    assert_eq!(Value::Bottom.join(&Value::Const(7)), Value::Const(7));
}

#[test]
fn partial_tuples_join_per_element_when_shapes_match() {
    let lhs = Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Const(2)]));
    let rhs = Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Const(3)]));
    let joined = Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Top]));

    assert_eq!(lhs.join(&rhs), joined);
}

#[test]
fn partial_tuples_join_to_top_when_arity_differs() {
    let lhs = Value::tuple(Product::from_vec(vec![Value::Const(1)]));
    let rhs = Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Const(2)]));

    assert_eq!(lhs.join(&rhs), Value::Top);
    assert_eq!(lhs.meet(&rhs), Value::Bottom);
}

#[test]
fn partial_structs_join_per_field_when_shapes_match() {
    let lhs = Value::struct_value(
        "point",
        vec![("x", Value::Const(1)), ("y", Value::Const(2))],
    );
    let rhs = Value::struct_value(
        "point",
        vec![("x", Value::Const(1)), ("y", Value::Const(3))],
    );
    let joined = Value::struct_value("point", vec![("x", Value::Const(1)), ("y", Value::Top)]);

    assert_eq!(lhs.join(&rhs), joined);
}

#[test]
fn partial_structs_join_to_top_when_shapes_differ() {
    let point = Value::struct_value("point", vec![("x", Value::Const(1))]);
    let size = Value::struct_value("size", vec![("x", Value::Const(1))]);

    assert_eq!(point.join(&size), Value::Top);
    assert_eq!(point.meet(&size), Value::Bottom);
}

#[test]
fn value_domain_satisfies_lattice_laws() {
    assert_finite_lattice_laws(&[
        Value::Bottom,
        Value::Const(1),
        Value::Const(2),
        Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Const(2)])),
        Value::tuple(Product::from_vec(vec![Value::Const(1), Value::Top])),
        Value::struct_value(
            "point",
            vec![("x", Value::Const(1)), ("y", Value::Const(2))],
        ),
        Value::struct_value("point", vec![("x", Value::Const(1)), ("y", Value::Top)]),
        Value::Top,
    ]);
}

#[test]
fn product_capability_uses_partial_tuple_internally() {
    let value = Value::from_product(Product::from_vec(vec![Value::Const(1), Value::Top]));

    assert!(matches!(value, Value::PartialTuple(_)));
    assert_eq!(value.as_product().map(Product::len), Some(2));
}

#[test]
fn scalar_ops_preserve_constants_and_lift_unknowns() {
    assert_eq!(Value::Const(2) + Value::Const(3), Value::Const(5));
    assert_eq!(Value::Const(2) * Value::Top, Value::Top);
    assert_eq!(Value::Bottom - Value::Const(3), Value::Bottom);
    assert_eq!(
        Value::Const(6).checked_div(Value::Const(3)),
        Some(Value::Const(2))
    );
    assert_eq!(
        Value::Const(6).checked_rem(Value::Const(4)),
        Some(Value::Const(2))
    );
    assert_eq!(
        Value::Const(1).checked_shl(Value::Const(3)),
        Some(Value::Const(8))
    );
    assert_eq!(
        Value::Const(8).checked_shr(Value::Const(1)),
        Some(Value::Const(4))
    );
}

#[test]
fn scalar_comparison_and_control_flow_traits_match_toy_constprop() {
    assert_eq!(Value::Const(2).cmp_lt(&Value::Const(3)), Value::Const(1));
    assert_eq!(Value::Const(0).is_truthy(), Some(false));
    assert_eq!(Value::Top.is_truthy(), None);
}
