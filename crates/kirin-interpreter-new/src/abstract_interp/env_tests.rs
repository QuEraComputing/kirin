use kirin_ir::{HasBottom, HasTop, Lattice, SSAValue, TestSSAValue};

use crate::{AbstractEnvStore, Env, ForkEnv};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TestValue(i64);

impl Lattice for TestValue {
    fn join(&self, other: &Self) -> Self {
        Self(self.0.max(other.0))
    }

    fn meet(&self, other: &Self) -> Self {
        Self(self.0.min(other.0))
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        self.0 <= other.0
    }
}

impl HasBottom for TestValue {
    fn bottom() -> Self {
        Self(0)
    }
}

impl HasTop for TestValue {
    fn top() -> Self {
        Self(i64::MAX)
    }
}

#[test]
fn fork_env_copies_values_without_aliasing() {
    let value = SSAValue::from(TestSSAValue(0));
    let mut store = AbstractEnvStore::new();
    let original = store.alloc();
    let forked = store.fork_env(original).unwrap();

    store.write(original, value, TestValue(1)).unwrap();
    store.write(forked, value, TestValue(2)).unwrap();

    assert_eq!(store.read(original, value).unwrap(), TestValue(1));
    assert_eq!(store.read(forked, value).unwrap(), TestValue(2));
}
