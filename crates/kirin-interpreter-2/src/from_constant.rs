use crate::InterpreterError;

/// Converts a compile-time constant value into a runtime interpreter value.
///
/// Bundles `TryFrom<T>` and its error conversion into a single trait,
/// eliminating the verbose `<V as TryFrom<T>>::Error: Error + Send + Sync`
/// bound from dialect impls.
pub trait FromConstant<T>: Sized {
    fn from_constant(value: T) -> Result<Self, InterpreterError>;
}

impl<V, T> FromConstant<T> for V
where
    V: TryFrom<T>,
    V::Error: std::error::Error + Send + Sync + 'static,
{
    fn from_constant(value: T) -> Result<Self, InterpreterError> {
        V::try_from(value).map_err(InterpreterError::custom)
    }
}
