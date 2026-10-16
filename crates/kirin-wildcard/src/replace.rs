use kirin::prelude::*;

/// Redirects every use of a matched value to a replacement value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$replace {matched} with {replacement}")]
#[kirin(builders, type = T)]
pub struct Replace<T: CompileTimeValue> {
    matched: SSAValue,
    replacement: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Replace<T> {
    /// The matched value whose uses are redirected.
    pub fn matched(&self) -> SSAValue {
        self.matched
    }

    /// The value that takes over the uses of [`Self::matched`].
    pub fn replacement(&self) -> SSAValue {
        self.replacement
    }
}
