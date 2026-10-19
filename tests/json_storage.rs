#![cfg(feature = "serde")]

use kirin::ir::{Arena, InternTable, Pipeline, Symbol};
use kirin_arith::ArithValue;

#[test]
fn numeric_constants_preserve_all_bits_and_integer_boundaries() {
    let values = vec![
        ArithValue::I8(i8::MIN),
        ArithValue::I16(i16::MIN),
        ArithValue::I32(i32::MIN),
        ArithValue::I64(i64::MIN),
        ArithValue::I128(i128::MIN),
        ArithValue::I128(i128::MAX),
        ArithValue::U8(u8::MAX),
        ArithValue::U16(u16::MAX),
        ArithValue::U32(u32::MAX),
        ArithValue::U64(u64::MAX),
        ArithValue::U128(u128::MAX),
        ArithValue::F32(-0.0),
        ArithValue::F64(-0.0),
        ArithValue::F32(f32::INFINITY),
        ArithValue::F32(f32::NEG_INFINITY),
        ArithValue::F64(f64::INFINITY),
        ArithValue::F64(f64::NEG_INFINITY),
        ArithValue::F32(f32::from_bits(0x7fc01234)),
        ArithValue::F64(f64::from_bits(0x7ff8000000001234)),
    ];
    let json = serde_json::to_string(&values).unwrap();
    let restored: Vec<ArithValue> = serde_json::from_str(&json).unwrap();
    assert_eq!(values, restored);
    assert!(!json.contains("null"));
}

#[test]
fn arena_tombstones_keep_slot_identity_and_next_allocation() {
    let mut arena: Arena<kirin::ir::Statement, String> = Arena::default();
    let deleted = arena.alloc("deleted".into());
    let live = arena.alloc("live".into());
    assert!(arena.delete(deleted));
    let next = arena.next_id();
    let mut restored: Arena<kirin::ir::Statement, String> =
        serde_json::from_str(&serde_json::to_string(&arena).unwrap()).unwrap();
    assert!(restored.get(deleted).unwrap().deleted());
    assert_eq!(&**restored.get(live).unwrap(), "live");
    assert_eq!(restored.iter().count(), 1);
    assert_eq!(restored.alloc("new".into()), next);
}

#[test]
fn intern_lookup_rebuilt_without_changing_ids() {
    let mut table: InternTable<String, Symbol> = InternTable::default();
    let alpha = table.intern("alpha".into());
    let beta = table.intern("beta".into());
    let mut restored: InternTable<String, Symbol> =
        serde_json::from_str(&serde_json::to_string(&table).unwrap()).unwrap();
    assert_eq!(restored.lookup("beta"), Some(beta));
    assert_eq!(restored.intern("alpha".into()), alpha);
    assert_ne!(restored.intern("gamma".into()), beta);
    assert!(serde_json::from_str::<InternTable<String, Symbol>>(r#"["same", "same"]"#).is_err());
}

#[test]
fn pipeline_rebuilds_function_lookup_for_all_stages() {
    let mut pipeline: Pipeline<String> = Pipeline::new();
    let source = pipeline.add_stage_raw("source".into());
    let target = pipeline.add_stage_raw("target".into());
    let function = pipeline.function().name("function").new().unwrap();
    // IDs for each stage can coincide; the compile-stage key disambiguates them.
    let sf: kirin::ir::StagedFunction = serde_json::from_str("0").unwrap();
    pipeline.link(function, source, sf).unwrap();
    pipeline.link(function, target, sf).unwrap();
    let mut restored: Pipeline<String> =
        serde_json::from_str(&serde_json::to_string(&pipeline).unwrap()).unwrap();
    assert_eq!(
        restored.resolve_staged_function("function", source),
        Some(sf)
    );
    assert_eq!(
        restored.resolve_staged_function("function", target),
        Some(sf)
    );
    assert!(restored.function().name("function").new().is_err());
}

#[test]
fn pipeline_positions_keep_their_stage_names() {
    use kirin::prelude::*;
    use kirin_test_languages::ArithFunctionLanguage;
    use kirin_test_utils::roundtrip::json_roundtrip;

    let mut ir: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let source = ir
        .add_stage()
        .stage(StageInfo::default())
        .name("source")
        .new();
    let target = ir
        .add_stage()
        .stage(StageInfo::default())
        .name("target")
        .new();

    // A position answers to its stage's own name, so the name must travel with
    // the stage.
    let restored = json_roundtrip(&ir);
    assert_eq!(restored.stage_by_name("source"), Some(source));
    assert_eq!(restored.stage_by_name("target"), Some(target));
}

#[test]
fn pipeline_refuses_to_serialize_a_poisoned_stage() {
    use kirin::ir::RewriteError;
    use kirin::prelude::*;
    use kirin_test_languages::ArithFunctionLanguage;

    let mut ir: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    let source = ir
        .add_stage()
        .stage(StageInfo::default())
        .name("source")
        .new();
    ir.add_stage()
        .stage(StageInfo::default())
        .name("target")
        .new();
    let _ = ir.run_pass::<ArithFunctionLanguage, _, (), _>(source, |_| {
        Err(RewriteError::CannotInsertTerminator)
    });

    // Skipping `source` instead would silently turn `target` into stage 0.
    let error = serde_json::to_string(&ir).expect_err("a poisoned stage has nothing to serialize");
    assert!(error.to_string().contains("Poisoned"), "{error}");
}

#[test]
fn finalized_multistage_specializations_preserve_invalidation() {
    use kirin::prelude::*;
    use kirin_test_languages::ArithFunctionLanguage;
    use kirin_test_utils::roundtrip::json_roundtrip;
    let text = r#"
stage @source fn @f(i64) -> i64;
stage @target fn @f(i64) -> i64;
specialize @source fn @f(i64) -> i64 cfg { ^entry(%x: i64) { ret %x; } }
specialize @target fn @f(i64) -> i64 cfg { ^entry(%x: i64) { ret %x; } }
"#;
    let mut ir: Pipeline<StageInfo<ArithFunctionLanguage>> = Pipeline::new();
    ir.parse(text).unwrap();
    let source = ir.stage_by_name("source").unwrap();
    let target = ir.stage_by_name("target").unwrap();
    let source_fn = ir.resolve_staged_function("f", source).unwrap();
    let target_fn = ir.resolve_staged_function("f", target).unwrap();
    let stage = ir.stage_mut(source).unwrap();
    let specialization = source_fn.expect_info(stage).specializations()[0].id();
    specialization.expect_info_mut(stage).invalidate();
    source_fn.expect_info_mut(stage).invalidate();
    let restored = json_roundtrip(&ir);
    assert!(
        source_fn
            .expect_info(restored.stage(source).unwrap())
            .is_invalidated()
    );
    assert!(
        specialization
            .expect_info(restored.stage(source).unwrap())
            .is_invalidated()
    );
    assert!(
        !target_fn
            .expect_info(restored.stage(target).unwrap())
            .is_invalidated()
    );
    assert_eq!(
        restored.resolve_staged_function("f", source),
        Some(source_fn)
    );
    assert_eq!(
        restored.resolve_staged_function("f", target),
        Some(target_fn)
    );
}
