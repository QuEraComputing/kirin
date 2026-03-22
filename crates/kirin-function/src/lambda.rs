use kirin::prelude::*;

/// A lambda expression that captures variables from the enclosing scope.
///
/// Use `#[wraps] Lambda(Lambda<T>)` in your language enum to delegate
/// parsing, printing, and interpretation to this type.
///
/// # Design: no explicit `Signature` field
///
/// Unlike [`FunctionBody`](super::FunctionBody), `Lambda` intentionally omits
/// a `sig: Signature<T>` field. The reasons are:
///
/// - **Parameters are implicit in block arguments.** A lambda's parameter types
///   are defined by the block arguments of its `body` region's entry block.
///   Duplicating them in a `Signature` would create a consistency hazard.
///
/// - **Return type is already present.** The `res: ResultValue` field carries
///   the lambda's return type, which is the only part of the signature that
///   cannot be recovered from the body region alone.
///
/// - **Captures are not part of the function type.** In PL theory, a closure's
///   *function type* describes its parameter and return types, not its captured
///   environment. The `captures` field is separate from the calling convention.
///
/// - **`FunctionBody` needs `sig` for top-level resolution.** Functions defined
///   at the pipeline level must declare their signature explicitly because the
///   caller resolves them by name, before seeing the body. Lambdas are inline
///   and their types flow from their definition site.
///
/// If your use case requires an explicit lambda signature (e.g., for a typed
/// intermediate representation where every term carries its type), define a
/// custom lambda type with a `sig: Signature<T>` field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "$lambda {name} captures({captures}) {body} -> {res:type}")]
pub struct Lambda<T: CompileTimeValue> {
    name: Symbol,
    captures: Vec<SSAValue>,
    pub(crate) body: Region,
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> HasRegionBody for Lambda<T> {
    fn region(&self) -> &Region {
        &self.body
    }
}
