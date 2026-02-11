use kirin::prelude::*;
use kirin::pretty::{Config, Document};
use kirin_arith::{ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_constant::Constant;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(fn, type = ArithType)]
enum NumericLanguage {
    Bitwise(Bitwise<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    ControlFlow(ControlFlow<ArithType>),
}

fn emit_bitwise_statement(
    input: &str,
    operands: &[(&str, ArithType)],
) -> (StageInfo<Bitwise<ArithType>>, Statement) {
    let mut stage: StageInfo<Bitwise<ArithType>> = StageInfo::default();

    for (name, ty) in operands {
        stage
            .ssa()
            .name((*name).to_string())
            .ty(*ty)
            .kind(SSAKind::Test)
            .new();
    }
    let statement =
        parse::<Bitwise<ArithType>>(input, &mut stage).expect("bitwise parse should succeed");
    (stage, statement)
}

fn render_bitwise_statement(stage: &StageInfo<Bitwise<ArithType>>, statement: Statement) -> String {
    let dialect = statement
        .get_info(stage)
        .expect("statement should exist")
        .definition();

    let doc = Document::new(Config::default(), stage);
    let mut output = String::new();
    dialect
        .pretty_print(&doc)
        .render_fmt(80, &mut output)
        .expect("render should succeed");
    output
}

fn assert_roundtrip(input: &str, operands: &[(&str, ArithType)], speculatable: bool) {
    let (stage, statement) = emit_bitwise_statement(input, operands);
    let dialect = statement
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(dialect.is_pure(), "bitwise statements should be pure");
    assert_eq!(
        dialect.is_speculatable(),
        speculatable,
        "unexpected speculatability for '{}'",
        input
    );
    assert_eq!(render_bitwise_statement(&stage, statement).trim(), input);
}

#[test]
fn test_roundtrip_all_operations_with_integer_types() {
    assert_roundtrip(
        "%ri_and = and %a, %b -> i32",
        &[("a", ArithType::I32), ("b", ArithType::I32)],
        true,
    );
    assert_roundtrip(
        "%ri_or = or %a, %b -> u64",
        &[("a", ArithType::U64), ("b", ArithType::U64)],
        true,
    );
    assert_roundtrip(
        "%ri_xor = xor %a, %b -> i8",
        &[("a", ArithType::I8), ("b", ArithType::I8)],
        true,
    );
    assert_roundtrip("%ri_not = not %a -> i16", &[("a", ArithType::I16)], true);
    assert_roundtrip(
        "%ri_shl = shl %a, %b -> u32",
        &[("a", ArithType::U32), ("b", ArithType::U32)],
        false,
    );
    assert_roundtrip(
        "%ri_shr = shr %a, %b -> i32",
        &[("a", ArithType::I32), ("b", ArithType::I32)],
        false,
    );
}

#[test]
fn test_shift_operations_are_pure_but_not_speculatable() {
    assert_roundtrip(
        "%rs_shl = shl %lhs, %rhs -> i64",
        &[("lhs", ArithType::I64), ("rhs", ArithType::I64)],
        false,
    );
    assert_roundtrip(
        "%rs_shr = shr %lhs, %rhs -> u32",
        &[("lhs", ArithType::U32), ("rhs", ArithType::U32)],
        false,
    );
}

#[test]
fn test_composes_with_constant_and_control_flow() {
    let mut stage: StageInfo<NumericLanguage> = StageInfo::default();
    let const_a = Constant::<ArithValue, ArithType>::new(&mut stage, ArithValue::I32(0b1010));
    let const_b = Constant::<ArithValue, ArithType>::new(&mut stage, ArithValue::I32(0b1100));
    let and_stmt = Bitwise::<ArithType>::op_and(&mut stage, const_a.result, const_b.result);
    let ret_stmt = ControlFlow::<ArithType>::op_return(&mut stage, and_stmt.result);

    let const_a_def = const_a
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(const_a_def, NumericLanguage::Constant(_)),
        "expected wrapped constant statement"
    );

    let const_b_def = const_b
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(const_b_def, NumericLanguage::Constant(_)),
        "expected wrapped constant statement"
    );

    let and_def = and_stmt
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(and_def, NumericLanguage::Bitwise(Bitwise::And { .. })),
        "expected wrapped bitwise::and statement"
    );
    assert!(and_def.is_pure(), "bitwise and should remain pure");
    assert!(
        and_def.is_speculatable(),
        "bitwise and should remain speculatable"
    );

    let ret_def = ret_stmt
        .id
        .get_info(&stage)
        .expect("statement should exist")
        .definition();
    assert!(
        matches!(
            ret_def,
            NumericLanguage::ControlFlow(ControlFlow::Return(_))
        ),
        "expected wrapped cf::ret statement"
    );
    assert!(
        ret_def.is_terminator(),
        "cf::ret should remain a terminator"
    );
}
