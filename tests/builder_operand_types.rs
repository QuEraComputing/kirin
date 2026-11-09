//! Generated builders check annotated `SSAValue` operands against their
//! `#[kirin(type = ...)]`, and skip operands whose type isn't set yet.

use kirin::prelude::*;
use kirin_ir::BuilderSSAKind;
use kirin_test_languages::{SimpleType, TypedOperandLanguage as L};

fn typed(stage: &mut BuilderStageInfo<L>, ty: SimpleType) -> SSAValue {
    stage.ssa().ty(ty).kind(BuilderSSAKind::Test).new()
}

#[test]
fn accepts_operand_of_annotated_type() {
    let mut stage = BuilderStageInfo::<L>::default();
    let idx = typed(&mut stage, SimpleType::I64);
    let any = typed(&mut stage, SimpleType::Bool); // unannotated, never checked
    L::op_index(&mut stage, idx, any);
}

#[test]
fn accepts_untyped_placeholders() {
    let mut stage = BuilderStageInfo::<L>::default();
    let idx = stage.block_argument().index(0);
    let port = stage.graph_port().index(0);
    L::op_index(&mut stage, idx, port);
    let port = stage.graph_port().name("x");
    L::op_cast(&mut stage, port, SimpleType::F32);
}

#[test]
#[should_panic(expected = "Index::field_0: expected operand of type I64, got F32")]
fn rejects_operand_of_wrong_type() {
    let mut stage = BuilderStageInfo::<L>::default();
    let idx = typed(&mut stage, SimpleType::F32);
    let any = typed(&mut stage, SimpleType::Bool);
    L::op_index(&mut stage, idx, any);
}

#[test]
#[should_panic(expected = "Cast::input: expected operand of type F32, got I64")]
fn rejects_operand_against_field_computed_type() {
    let mut stage = BuilderStageInfo::<L>::default();
    let input = typed(&mut stage, SimpleType::I64);
    L::op_cast(&mut stage, input, SimpleType::F32);
}
