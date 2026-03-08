/// A function signature parameterized over the type `T` and optional constraints `C`.
///
/// - `params`: the parameter types of the function.
/// - `ret`: the return type.
/// - `constraints`: optional constraint context (e.g., type-variable bindings). Defaults to `()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature<T, C = ()> {
    pub params: Vec<T>,
    pub ret: T,
    pub constraints: C,
}

impl<T: crate::Placeholder> Signature<T> {
    /// Creates a signature with no parameters and a placeholder return type.
    ///
    /// Use this when constructing IR before type inference has resolved types.
    pub fn placeholder() -> Self {
        Signature {
            params: Vec::new(),
            ret: T::placeholder(),
            constraints: (),
        }
    }
}
