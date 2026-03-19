/// A function signature parameterized over the type `T` and optional constraints `C`.
///
/// - `params`: the parameter types of the function.
/// - `ret`: the return type.
/// - `constraints`: optional constraint context (e.g., type-variable bindings). Defaults to `()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature<T, C = ()> {
    params: Vec<T>,
    ret: T,
    constraints: C,
}

impl<T, C> Signature<T, C> {
    /// Creates a new signature with the given parameters, return type, and constraints.
    pub fn new(params: Vec<T>, ret: T, constraints: C) -> Self {
        Signature {
            params,
            ret,
            constraints,
        }
    }

    /// Returns the parameter types of the signature.
    pub fn params(&self) -> &[T] {
        &self.params
    }

    /// Returns the return type of the signature.
    pub fn ret(&self) -> &T {
        &self.ret
    }

    /// Returns the constraints of the signature.
    pub fn constraints(&self) -> &C {
        &self.constraints
    }
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
