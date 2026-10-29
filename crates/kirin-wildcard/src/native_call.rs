use kirin::prelude::*;

/// Calls the native Rust function registered under a name.
///
/// In a [`Pattern`](crate::Pattern) body it computes values from the matched
/// ones, such as `is_power_of_two(%c)`, for a [`Constraint`](crate::Constraint)
/// to check. It cannot build IR outside a [`Rewrite`](crate::Rewrite) body,
/// since the match may still fail.
///
/// In a [`Rewrite`](crate::Rewrite) body it computes its results, such as the
/// shift amount `log2(%c)`, and may also act on the IR itself, erasing or
/// replacing ops. The pattern stores only the name and the arguments; the
/// function itself is looked up when the rule runs.
///
/// The name is a plain string, not a [`Symbol`]: nothing in the IR defines
/// the function, so it must not read as an `@` reference to an IR entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$native_call {name}({args})[ -> {results:type}]")]
#[kirin(builders, type = T)]
pub struct NativeCall<T: CompileTimeValue> {
    name: String,
    args: Vec<SSAValue>,
    results: Vec<ResultValue>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> NativeCall<T> {
    /// The name the native function is registered under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The values passed to the native function, in order.
    pub fn args(&self) -> &[SSAValue] {
        &self.args
    }
}
