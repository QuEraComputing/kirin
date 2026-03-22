use kirin::prelude::*;

/// Structural function-body statement used by function text parsing.
///
/// The `sig` field stores the function's type signature (`(T, T) -> T`),
/// parsed from the format string. `derive(Dialect)` generates `HasSignature`
/// which returns `Some(self.sig.clone())`.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "fn {:name}{sig} {body}")]
pub struct FunctionBody<T: CompileTimeValue> {
    pub(crate) body: Region,
    pub(crate) sig: Signature<T>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> HasRegionBody for FunctionBody<T> {
    fn region(&self) -> &Region {
        &self.body
    }
}
