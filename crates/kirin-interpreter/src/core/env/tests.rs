use std::convert::Infallible;

use kirin_ir::{SSAValue, TestSSAValue};

use super::EnvStore;
use crate::InterpreterError;

/// The concrete engine's container shape: no context identity at all.
type Anonymous = EnvStore<Infallible, SSAValue, i64>;

/// A keyed container standing in for an analysis that keys environments by
/// context (a `(stage, function)` pair here, spelled as a plain integer).
type Keyed = EnvStore<u8, SSAValue, i64>;

fn ssa(index: usize) -> SSAValue {
    SSAValue::from(TestSSAValue(index))
}

#[test]
fn alloc_is_always_fresh_but_equal_keys_reuse_one_environment() {
    let mut env = Anonymous::new();
    assert_ne!(env.alloc(), env.alloc());

    let mut env = Keyed::new();
    let first = env.get_or_allocate(1);
    assert_eq!(env.get_or_allocate(1), first);
    assert_eq!(env.context_env(&1), Some(first));
    // An unkeyed allocation is never handed out for a key, and vice versa.
    assert_ne!(env.alloc(), first);
    assert_ne!(env.get_or_allocate(2), first);
}

#[test]
fn distinct_contexts_isolate_the_same_anchor() {
    let mut env = Keyed::new();
    let first = env.get_or_allocate(1);
    let second = env.get_or_allocate(2);
    let value = ssa(0);

    env.write(first, value, 2).unwrap();
    env.write(second, value, 9).unwrap();

    assert_eq!(env.read(first, value), Ok(Some(2)));
    assert_eq!(env.read(second, value), Ok(Some(9)));
}

#[test]
fn assignment_replaces_the_stored_value() {
    let mut env = Keyed::new();
    let index = env.get_or_allocate(1);
    let value = ssa(0);

    env.write(index, value, 2).unwrap();
    env.write(index, value, 5).unwrap();

    // Storage assigns; merging a new value into an old one is an analysis
    // decision made above this layer.
    assert_eq!(env.read(index, value), Ok(Some(5)));
}

#[test]
fn freeing_a_key_releases_it_for_a_fresh_environment() {
    let mut env = Keyed::new();
    let first = env.get_or_allocate(1);
    let value = ssa(0);
    env.write(first, value, 7).unwrap();

    env.free(first).unwrap();

    assert_eq!(env.context_env(&1), None);
    let second = env.get_or_allocate(1);
    assert_ne!(second, first);
    // Freed indices are not reused, so the old facts cannot leak into the new
    // environment.
    assert_eq!(env.read(second, value), Ok(None));
}

#[test]
fn freed_environments_stay_invalid() {
    let mut env = Keyed::new();
    let index = env.get_or_allocate(1);

    env.free(index).unwrap();

    assert_eq!(
        env.free(index),
        Err(InterpreterError::InvalidEnvIndex(index))
    );
    assert_eq!(
        env.read(index, ssa(0)),
        Err(InterpreterError::InvalidEnvIndex(index))
    );
    assert_eq!(
        env.write(index, ssa(0), 1),
        Err(InterpreterError::InvalidEnvIndex(index))
    );
}

#[test]
fn an_invalid_handle_is_distinguishable_from_an_absent_anchor() {
    let mut env = Keyed::new();
    let index = env.get_or_allocate(1);
    let missing = ssa(0);

    // Live environment, nothing stored: absent, not an error. The engine — not
    // storage — decides whether that is an unbound-value error or bottom.
    assert_eq!(env.read(index, missing), Ok(None));
    assert!(!env.environment(index).unwrap().contains(missing));

    env.free(index).unwrap();

    assert_eq!(
        env.read(index, missing),
        Err(InterpreterError::InvalidEnvIndex(index))
    );
    assert!(env.environment(index).is_err());
}
