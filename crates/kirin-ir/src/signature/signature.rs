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

impl<T: Default> Default for Signature<T> {
    fn default() -> Self {
        Signature {
            params: Vec::new(),
            ret: T::default(),
            constraints: (),
        }
    }
}
