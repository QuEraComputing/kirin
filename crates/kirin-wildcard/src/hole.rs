use kirin::prelude::*;

/// A hole: matches any SSA value and binds it to its result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$hole -> {result:type}")]
#[kirin(builders, type = T)]
pub struct Hole<T: CompileTimeValue> {
    result: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Hole<T> {
    /// The pattern value bound to whatever the hole matches.
    pub fn result(&self) -> ResultValue {
        self.result
    }
}
