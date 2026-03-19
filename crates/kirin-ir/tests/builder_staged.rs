//! Integration tests for staged function policies, specialization, redefine,
//! and signature matching.

mod common;

use common::{BuilderDialect, TestType, new_stage};
use kirin_ir::*;

fn sig(ty: TestType) -> Signature<TestType> {
    Signature::new(vec![ty.clone()], ty, ())
}

// --- Staged function name policy tests ---

#[test]
fn staged_name_policy_defaults_to_single_interface() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage = new_stage();
    assert_eq!(
        stage.staged_name_policy(),
        StagedNamePolicy::SingleInterface
    );

    stage
        .staged_function()
        .name(foo)
        .signature(sig(TestType::I32))
        .new()
        .expect("first staged function should be created");

    let err = stage
        .staged_function()
        .name(foo)
        .signature(sig(TestType::I64))
        .new()
        .expect_err("same name + different signature should fail by default");

    assert_eq!(
        err.conflict_kind,
        StagedFunctionConflictKind::SignatureMismatchUnderSingleInterface
    );
}

#[test]
fn staged_name_policy_multiple_dispatch_allows_different_signatures() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage = new_stage();
    stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);

    stage
        .staged_function()
        .name(foo)
        .signature(sig(TestType::I32))
        .new()
        .expect("first staged function should be created");

    stage
        .staged_function()
        .name(foo)
        .signature(sig(TestType::I64))
        .new()
        .expect("same name + different signature should be allowed under MultipleDispatch");
}

#[test]
fn duplicate_signature_is_rejected_even_with_multiple_dispatch() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage = new_stage();
    stage.set_staged_name_policy(StagedNamePolicy::MultipleDispatch);

    let i32_sig = sig(TestType::I32);
    stage
        .staged_function()
        .name(foo)
        .signature(i32_sig.clone())
        .new()
        .expect("first staged function should be created");

    let err = stage
        .staged_function()
        .name(foo)
        .signature(i32_sig)
        .new()
        .expect_err("duplicate (name, signature) should still fail");

    assert_eq!(
        err.conflict_kind,
        StagedFunctionConflictKind::DuplicateSignature
    );
}

// --- Specialize / Redefine tests ---

#[test]
fn specialize_success_and_duplicate_error() {
    let mut stage = new_stage();

    let sf = stage.staged_function().new().unwrap();

    let body1 = stage.statement().definition(BuilderDialect::Return).new();
    let _spec1 = stage
        .specialize()
        .staged_func(sf)
        .body(body1)
        .new()
        .expect("first specialize should succeed");

    let body2 = stage.statement().definition(BuilderDialect::Return).new();
    let err = stage
        .specialize()
        .staged_func(sf)
        .body(body2)
        .new()
        .expect_err("duplicate signature should fail");

    assert_eq!(err.conflicting.len(), 1);
}

#[test]
fn redefine_specialization_invalidates_and_registers() {
    let mut stage = new_stage();
    let sf = stage.staged_function().new().unwrap();

    let body1 = stage.statement().definition(BuilderDialect::Return).new();
    let spec1 = stage
        .specialize()
        .staged_func(sf)
        .body(body1)
        .new()
        .unwrap();

    let body2 = stage.statement().definition(BuilderDialect::Return).new();
    let err = stage
        .specialize()
        .staged_func(sf)
        .body(body2)
        .new()
        .expect_err("duplicate");

    let spec2 = stage.redefine_specialization(err);

    let stage = stage.finalize().unwrap();
    let old = spec1.get_info(&stage).unwrap();
    assert!(old.is_invalidated());

    let new = spec2.get_info(&stage).unwrap();
    assert!(!new.is_invalidated());
}

#[test]
fn redefine_staged_function_invalidates_and_registers() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage = new_stage();

    let sf1 = stage.staged_function().name(foo).new().unwrap();

    let err = stage
        .staged_function()
        .name(foo)
        .new()
        .expect_err("duplicate signature");

    let sf2 = stage.redefine_staged_function(err);

    let stage = stage.finalize().unwrap();
    let old_info = sf1.get_info(&stage).unwrap();
    assert!(old_info.is_invalidated());

    let new_info = sf2.get_info(&stage).unwrap();
    assert!(!new_info.is_invalidated());
}

// --- StagedFunctionInfo::all_matching tests ---

#[test]
fn staged_function_all_matching_returns_most_specific() {
    let mut stage = new_stage();
    let sf = stage.staged_function().new().unwrap();

    let body1 = stage.statement().definition(BuilderDialect::Return).new();
    let sig_i32 = Signature::new(vec![TestType::I32], TestType::Any, ());
    let _spec1 = stage
        .specialize()
        .staged_func(sf)
        .signature(sig_i32.clone())
        .body(body1)
        .new()
        .unwrap();

    let body2 = stage.statement().definition(BuilderDialect::Return).new();
    let sig_i64 = Signature::new(vec![TestType::I64], TestType::Any, ());
    let _spec2 = stage
        .specialize()
        .staged_func(sf)
        .signature(sig_i64)
        .body(body2)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();

    let matches = sf_info.all_matching::<ExactSemantics>(&sig_i32);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0.signature(), &sig_i32);
}

#[test]
fn staged_function_all_matching_excludes_invalidated() {
    let mut stage = new_stage();
    let sf = stage.staged_function().new().unwrap();

    let body1 = stage.statement().definition(BuilderDialect::Return).new();
    let default_sig: Signature<TestType> = Signature::placeholder();
    let spec1 = stage
        .specialize()
        .staged_func(sf)
        .body(body1)
        .new()
        .unwrap();

    // Invalidate spec1 by redefining
    let body2 = stage.statement().definition(BuilderDialect::Return).new();
    let err = stage
        .specialize()
        .staged_func(sf)
        .body(body2)
        .new()
        .expect_err("duplicate");
    let _spec2 = stage.redefine_specialization(err);

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();
    let matches = sf_info.all_matching::<ExactSemantics>(&default_sig);

    assert_eq!(matches.len(), 1);

    let old = spec1.get_info(&stage).unwrap();
    assert!(old.is_invalidated());
}
