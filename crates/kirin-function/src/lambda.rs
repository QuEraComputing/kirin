use kirin::prelude::*;

/// A lambda expression that captures variables from the enclosing scope.
///
/// Use `#[wraps] Lambda(Lambda<T>)` in your language enum to delegate
/// parsing, printing, and interpretation to this type.
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
