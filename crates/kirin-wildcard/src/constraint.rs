use kirin::prelude::*;

/// Fails the match unless a value computed in the pattern body is true.
///
/// The value comes from a [`NativeCall`](crate::NativeCall), so a rule's
/// `is_power_of_two(%c)` becomes a `native_call` followed by a `constraint`
/// on its result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$constraint {success}")]
#[kirin(builders, type = T)]
pub struct Constraint<T: CompileTimeValue> {
    success: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Constraint<T> {
    /// The value indicating the success of the constraint.
    pub fn success(&self) -> SSAValue {
        self.success
    }
}
