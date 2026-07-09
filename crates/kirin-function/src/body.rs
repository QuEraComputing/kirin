use kirin::prelude::*;

/// Structural function-body statement used by function text parsing.
///
/// The `sig` field stores the function's type signature (`(T, T) -> T`),
/// parsed from the format string. `derive(Dialect)` generates `HasSignature`
/// which returns `Some(self.sig.clone())`.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "fn {:name}{sig} {body}")]
pub struct Function<T: CompileTimeValue> {
    pub(crate) body: Cfg,
    pub(crate) sig: Signature<T>,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> HasCfgBody for Function<T> {
    fn cfg(&self) -> &Cfg {
        &self.body
    }
}
