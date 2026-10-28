//! The terminators of a [`Rewrite`](crate::Rewrite) body, which tell the
//! rewrite driver how to go on walking after the rewrite runs. A body
//! without one advances.

use kirin::prelude::*;

/// Go on walking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$advance")]
#[kirin(terminator, builders, type = T)]
pub struct Advance<T: CompileTimeValue> {
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Go on walking, without walking the statements nested in the root.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$skip")]
#[kirin(terminator, builders, type = T)]
pub struct Skip<T: CompileTimeValue> {
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// End the current walk.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$interrupt")]
#[kirin(terminator, builders, type = T)]
pub struct Interrupt<T: CompileTimeValue> {
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

/// Go on as `value` says. The value comes from a
/// [`NativeCall`](crate::NativeCall) in the same rewrite body, whose function
/// decides how to go on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$walk_result {value}")]
#[kirin(terminator, builders, type = T)]
pub struct WalkResultOf<T: CompileTimeValue> {
    value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> WalkResultOf<T> {
    /// The value saying how to go on.
    pub fn value(&self) -> SSAValue {
        self.value
    }
}
