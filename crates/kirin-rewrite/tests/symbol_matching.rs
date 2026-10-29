//! Symbol attributes must be compared through the rule and program tables.

use std::{cell::Cell, rc::Rc};

use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::Call;
use kirin_ir::walk::PostOrder;
use kirin_rewrite::{NativeRegistry, RewriteDriver, RuleSet};
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::{NativeCall, Pattern, PatternLanguage, Rewrite};

/// Build the real named-call operation without introducing operands or results.
fn named_call(target: Symbol) -> L {
    let mut stage = BuilderStageInfo::<L>::default();
    let call = Call::<ArithType>::build(&mut stage).named(target).insert();
    stage.statement_arena()[call.id].definition().clone()
}

/// Count matches of a call to `foo` in a program with the supplied symbol table.
fn matches_call(symbols: &[&str], target: usize) -> usize {
    let matches = Rc::new(Cell::new(0));
    let recorded = matches.clone();
    let natives = NativeRegistry::new().action("record", move |_, _| {
        recorded.set(recorded.get() + 1);
        Ok(vec![])
    });
    let rules = RuleSet::build(natives, |stage| {
        let target = stage.symbol_table_mut().intern("foo".to_string());
        let op = stage
            .statement()
            .definition(PatternLanguage::Op(named_call(target)))
            .new();
        let action = NativeCall::<ArithType>::new(stage, 0, "record".to_string(), vec![]);
        let actions = stage.block().stmt(action.id).new();
        let rewrite = Rewrite::<ArithType>::new(stage, op, actions);
        let body = stage.block().stmt(op).terminator(rewrite.id).new();
        let name = stage.symbol_table_mut().intern("match_foo".to_string());
        vec![Pattern::<ArithType>::new(stage, name, body).id]
    })
    .unwrap();

    let mut builder = BuilderStageInfo::<L>::default();
    for &name in symbols {
        builder.symbol_table_mut().intern(name.to_string());
    }
    let root = builder
        .statement()
        .definition(named_call(Symbol::from(target)))
        .new();
    let mut stage = builder.finalize().unwrap();
    RewriteDriver::new(&rules)
        .run_at::<PostOrder>(&mut Rewriter::new(&mut stage), root)
        .unwrap();
    matches.get()
}

#[test]
fn different_names_with_equal_ids_do_not_match() {
    assert_eq!(matches_call(&["bar"], 0), 0);
}

#[test]
fn equal_names_with_different_ids_match() {
    assert_eq!(matches_call(&["bar", "foo"], 1), 1);
}

#[test]
fn equal_names_with_equal_ids_match() {
    assert_eq!(matches_call(&["foo"], 0), 1);
}

#[test]
fn an_unresolved_symbol_does_not_match() {
    assert_eq!(matches_call(&[], 0), 0);
}
