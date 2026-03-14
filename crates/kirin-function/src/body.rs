use kirin::prelude::*;

/// Structural function-body statement used by function text parsing.
///
/// Name, signature, and return type live on staged/specialized function headers.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "{body}")]
pub struct FunctionBody<T: CompileTimeValue> {
    pub(crate) body: Region,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
