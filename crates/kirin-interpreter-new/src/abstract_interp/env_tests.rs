use kirin_ir::{SSAValue, TestSSAValue};

use crate::{AbstractEnvStore, AbstractValue, Env, ForkEnv};

impl AbstractValue for i64 {
    fn bottom() -> Self {
        0
    }

    fn top() -> Self {
        i64::MAX
    }

    fn join(&self, other: &Self) -> Self {
        self.max(other).to_owned()
    }
}

#[test]
fn fork_env_copies_values_without_aliasing() {
    let value = SSAValue::from(TestSSAValue(0));
    let mut store = AbstractEnvStore::new();
    let original = store.alloc();
    let forked = store.fork_env(original).unwrap();

    store.write(original, value, 1).unwrap();
    store.write(forked, value, 2).unwrap();

    assert_eq!(store.read(original, value).unwrap(), 1);
    assert_eq!(store.read(forked, value).unwrap(), 2);
}
