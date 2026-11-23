//! Integration tests for staged function policies, specialization, redefine,
//! and signature matching.

mod common;

use common::{BuilderDialect, TestType, any_sig, define, new_stage};
use kirin_ir::*;

fn sig(ty: TestType) -> Signature<TestType> {
    Signature::new().takes([ty.clone()]).returns(ty)
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

    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let definition1 = define(&mut stage, any_sig());
    let _spec1 = stage
        .specialize()
        .staged_func(sf)
        .definition(definition1)
        .new()
        .expect("first specialize should succeed");

    let definition2 = define(&mut stage, any_sig());
    let Err(SpecializeError::Duplicate(err)) = stage
        .specialize()
        .staged_func(sf)
        .definition(definition2)
        .new()
    else {
        panic!("duplicate signature should fail");
    };

    assert_eq!(err.conflicting.len(), 1);
}

#[test]
fn redefine_specialization_invalidates_and_registers() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let definition1 = define(&mut stage, any_sig());
    let spec1 = stage
        .specialize()
        .staged_func(sf)
        .definition(definition1)
        .new()
        .unwrap();

    let definition2 = define(&mut stage, any_sig());
    let Err(SpecializeError::Duplicate(err)) = stage
        .specialize()
        .staged_func(sf)
        .definition(definition2)
        .new()
    else {
        panic!("duplicate");
    };

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

    let sf1 = stage
        .staged_function()
        .name(foo)
        .signature(any_sig())
        .new()
        .unwrap();
    let definition = define(&mut stage, any_sig());
    let spec1 = stage
        .specialize()
        .staged_func(sf1)
        .definition(definition)
        .new()
        .unwrap();

    let err = stage
        .staged_function()
        .name(foo)
        .signature(any_sig())
        .new()
        .expect_err("duplicate signature");

    let sf2 = stage.redefine_staged_function(err);

    let stage = stage.finalize().unwrap();
    let old_info = sf1.get_info(&stage).unwrap();
    assert!(old_info.is_invalidated());
    // The old function's specializations go with it.
    assert!(spec1.get_info(&stage).unwrap().is_invalidated());

    let new_info = sf2.get_info(&stage).unwrap();
    assert!(!new_info.is_invalidated());
}

#[test]
fn specialize_rejects_an_invalidated_staged_function() {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let foo = gs.intern("foo".to_string());
    let mut stage = new_stage();

    let sf1 = stage
        .staged_function()
        .name(foo)
        .signature(any_sig())
        .new()
        .unwrap();
    let err = stage
        .staged_function()
        .name(foo)
        .signature(any_sig())
        .new()
        .expect_err("duplicate signature");
    stage.redefine_staged_function(err);

    let definition = define(&mut stage, any_sig());
    let result = stage
        .specialize()
        .staged_func(sf1)
        .definition(definition)
        .new();
    assert!(matches!(
        result,
        Err(SpecializeError::InvalidatedStagedFunction(sf)) if sf == sf1
    ));

    let stage = stage.finalize().unwrap();
    assert!(sf1.get_info(&stage).unwrap().specializations().is_empty());
}

// --- Signature ownership ---

#[test]
fn specialization_reads_its_signature_from_the_definition() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();
    let sig_i32 = Signature::new()
        .takes([TestType::I32])
        .returns(TestType::I32);
    let definition = define(&mut stage, sig_i32.clone());
    let spec = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    // The interface stays the staged function's own.
    assert_eq!(sf.get_info(&stage).unwrap().signature(), &any_sig());
    let info = spec.get_info(&stage).unwrap();
    assert_eq!(info.signature(&stage), Some(&sig_i32));
    assert_eq!(
        info.return_types(&stage).map(Product::as_slice),
        Some(&[TestType::I32][..])
    );
}

#[test]
fn specialize_rejects_a_definition_without_a_signature() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();
    let definition = stage.statement().definition(BuilderDialect::Return).new();
    let result = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new();
    assert!(matches!(
        result,
        Err(SpecializeError::MissingSignature(stmt)) if stmt == definition
    ));
}

#[test]
fn specialize_rejects_a_statement_that_is_not_in_the_stage() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();
    let unallocated = stage.statement_arena().next_id();
    let result = stage
        .specialize()
        .staged_func(sf)
        .definition(unallocated)
        .new();
    assert!(matches!(
        result,
        Err(SpecializeError::UnknownDefinition(stmt)) if stmt == unallocated
    ));
}

#[test]
fn specialize_rejects_a_signature_that_does_not_fit_the_interface_shape() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    // One parameter too many.
    let wide = Signature::new()
        .takes([TestType::I32, TestType::I32])
        .returns(TestType::Any);
    let definition = define(&mut stage, wide);
    let Err(SpecializeError::InterfaceMismatch(err)) = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new()
    else {
        panic!("a two-parameter definition does not fit a one-parameter interface");
    };
    assert_eq!((err.params, err.rets), ((1, 2), (1, 1)));

    // One return too many.
    let pair = Signature::new()
        .takes([TestType::I32])
        .returns_each([TestType::Any, TestType::Any]);
    let definition = define(&mut stage, pair);
    let Err(SpecializeError::InterfaceMismatch(err)) = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new()
    else {
        panic!("a two-return definition does not fit a one-return interface");
    };
    assert_eq!((err.params, err.rets), ((1, 1), (1, 2)));
}

/// The default `ShapeSemantics` compares counts only, so specializations of
/// unrelated types share one interface.
#[test]
fn shape_semantics_accepts_any_types_of_the_interface_shape() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    for ty in [TestType::I32, TestType::I64, TestType::Qubit] {
        let definition = define(&mut stage, Signature::new().takes([ty.clone()]).returns(ty));
        stage
            .specialize()
            .staged_func(sf)
            .definition(definition)
            .new()
            .expect("same shape fits under ShapeSemantics");
    }
}

// --- Editing existing specializations ---

/// A specialization superseded by one with a different signature is not a
/// duplicate, so it is retired explicitly.
#[test]
fn invalidate_specialization_retires_it_from_dispatch() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let wide = define(&mut stage, any_sig());
    let wide = stage
        .specialize()
        .staged_func(sf)
        .definition(wide)
        .new()
        .unwrap();
    let narrow = define(
        &mut stage,
        Signature::new()
            .takes([TestType::Any])
            .returns(TestType::I32),
    );
    let narrow = stage
        .specialize()
        .staged_func(sf)
        .definition(narrow)
        .new()
        .unwrap();

    stage.invalidate_specialization(wide).unwrap();
    // Invalidating twice changes nothing.
    stage.invalidate_specialization(wide).unwrap();

    let stage = stage.finalize().unwrap();
    assert!(wide.get_info(&stage).unwrap().is_invalidated());
    assert!(!narrow.get_info(&stage).unwrap().is_invalidated());
    assert_eq!(
        sf.get_info(&stage).unwrap().unique_live_specialization(),
        Ok(narrow)
    );
}

#[test]
fn add_specialization_backedge_records_each_caller_once() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();
    let callee = define(&mut stage, any_sig());
    let callee = stage
        .specialize()
        .staged_func(sf)
        .definition(callee)
        .new()
        .unwrap();
    let caller = define(
        &mut stage,
        Signature::new()
            .takes([TestType::I32])
            .returns(TestType::Any),
    );
    let caller = stage
        .specialize()
        .staged_func(sf)
        .definition(caller)
        .new()
        .unwrap();

    stage.add_specialization_backedge(callee, caller).unwrap();
    stage.add_specialization_backedge(callee, caller).unwrap();

    let stage = stage.finalize().unwrap();
    assert_eq!(callee.get_info(&stage).unwrap().backedges(), &[caller]);
    assert!(caller.get_info(&stage).unwrap().backedges().is_empty());
}

#[test]
fn specialization_edits_reject_an_unknown_specialization() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();
    let definition = define(&mut stage, any_sig());
    let known = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new()
        .unwrap();

    // Build an id past the end by specializing in a throwaway stage.
    let mut other = new_stage();
    let other_sf = other.staged_function().signature(any_sig()).new().unwrap();
    let mut unknown = known;
    for ty in [TestType::Any, TestType::I32] {
        let definition = define(
            &mut other,
            Signature::new().takes([ty]).returns(TestType::Any),
        );
        unknown = other
            .specialize()
            .staged_func(other_sf)
            .definition(definition)
            .new()
            .unwrap();
    }
    assert_ne!(unknown, known);

    assert_eq!(
        stage.invalidate_specialization(unknown),
        Err(UnknownSpecialization(unknown))
    );
    assert_eq!(
        stage.add_specialization_backedge(known, unknown),
        Err(UnknownSpecialization(unknown))
    );
    assert_eq!(
        stage.add_specialization_backedge(unknown, known),
        Err(UnknownSpecialization(unknown))
    );
}

// --- StagedFunctionInfo::all_matching tests ---

#[test]
fn staged_function_all_matching_returns_most_specific() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let sig_i32 = Signature::new()
        .takes([TestType::I32])
        .returns(TestType::Any);
    let definition1 = define(&mut stage, sig_i32.clone());
    let _spec1 = stage
        .specialize()
        .staged_func(sf)
        .definition(definition1)
        .new()
        .unwrap();

    let sig_i64 = Signature::new()
        .takes([TestType::I64])
        .returns(TestType::Any);
    let definition2 = define(&mut stage, sig_i64.clone());
    let _spec2 = stage
        .specialize()
        .staged_func(sf)
        .definition(definition2)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();

    let matches = sf_info.all_matching(&stage, &sig_i32);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].0.signature(&stage), Some(&sig_i32));
}

#[test]
fn staged_function_all_matching_excludes_invalidated() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let definition1 = define(&mut stage, any_sig());
    let default_sig = any_sig();
    let spec1 = stage
        .specialize()
        .staged_func(sf)
        .definition(definition1)
        .new()
        .unwrap();

    // Invalidate spec1 by redefining
    let definition2 = define(&mut stage, any_sig());
    let Err(SpecializeError::Duplicate(err)) = stage
        .specialize()
        .staged_func(sf)
        .definition(definition2)
        .new()
    else {
        panic!("duplicate");
    };
    let _spec2 = stage.redefine_specialization(err);

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();
    let matches = sf_info.all_matching(&stage, &default_sig);

    assert_eq!(matches.len(), 1);

    let old = spec1.get_info(&stage).unwrap();
    assert!(old.is_invalidated());
}

#[test]
fn staged_function_unique_live_specialization_returns_only_live_specialization() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let definition = define(&mut stage, any_sig());
    let spec = stage
        .specialize()
        .staged_func(sf)
        .definition(definition)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();

    assert_eq!(sf_info.unique_live_specialization().unwrap(), spec);
}

#[test]
fn staged_function_unique_live_specialization_rejects_ambiguous_live_set() {
    let mut stage = new_stage();
    let sf = stage.staged_function().signature(any_sig()).new().unwrap();

    let sig_i32 = Signature::new()
        .takes([TestType::I32])
        .returns(TestType::Any);
    let definition1 = define(&mut stage, sig_i32.clone());
    stage
        .specialize()
        .staged_func(sf)
        .definition(definition1)
        .new()
        .unwrap();

    let sig_i64 = Signature::new()
        .takes([TestType::I64])
        .returns(TestType::Any);
    let definition2 = define(&mut stage, sig_i64.clone());
    stage
        .specialize()
        .staged_func(sf)
        .definition(definition2)
        .new()
        .unwrap();

    let stage = stage.finalize().unwrap();
    let sf_info = sf.get_info(&stage).unwrap();

    assert_eq!(
        sf_info.unique_live_specialization(),
        Err(UniqueLiveSpecializationError::Ambiguous { count: 2 })
    );
}

#[test]
fn program_resolve_function_uses_stage_local_symbol_table() {
    let mut program: Program<StageInfo<BuilderDialect>> = Program::new();
    let stage_id = program
        .add_stage()
        .stage(StageInfo::default())
        .name("test")
        .new();
    let func = program.function().name("target").new().unwrap();
    let target = {
        let stage = program.stage_mut(stage_id).unwrap();
        stage.symbol_table_mut().intern("target".to_string())
    };

    let stage = program.stage(stage_id).unwrap();
    assert_eq!(program.resolve_function(stage, target), Some(func));
}
