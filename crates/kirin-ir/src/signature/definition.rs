/// A function signature parameterized over the type `T` and optional constraints `C`.
///
/// - `params`: the parameter types of the function.
/// - `rets`: the return types.
/// - `constraints`: optional constraint context (e.g., type-variable bindings). Defaults to `()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Signature<T, C = ()> {
    params: Vec<T>,
    rets: Vec<T>,
    constraints: C,
}

impl<T, C> Signature<T, C> {
    /// Creates a new signature with a single return type.
    pub fn new(params: Vec<T>, ret: T, constraints: C) -> Self {
        Self::with_rets(params, vec![ret], constraints)
    }

    /// Creates a new signature with several return types.
    pub fn with_rets(params: Vec<T>, rets: Vec<T>, constraints: C) -> Self {
        Signature {
            params,
            rets,
            constraints,
        }
    }

    /// Returns the parameter types of the signature.
    pub fn params(&self) -> &[T] {
        &self.params
    }

    /// Returns the return types of the signature.
    pub fn rets(&self) -> &[T] {
        &self.rets
    }

    /// Returns the constraints of the signature.
    pub fn constraints(&self) -> &C {
        &self.constraints
    }

    /// Formats the return types as `T`, `(T, U)`, or `None`; a lone `(`-type is wrapped.
    pub fn rets_to_string(&self) -> String
    where
        T: std::fmt::Display,
    {
        match self.rets.as_slice() {
            [] => "None".to_string(),
            [ret] => match ret.to_string() {
                ret if ret.starts_with('(') => format!("({ret})"),
                ret => ret,
            },
            rets => {
                let rets: Vec<String> = rets.iter().map(ToString::to_string).collect();
                format!("({})", rets.join(", "))
            }
        }
    }
}

impl<T: std::fmt::Display, C> std::fmt::Display for Signature<T, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(")?;
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", param)?;
        }
        write!(f, ") -> {}", self.rets_to_string())
    }
}

impl<T: crate::Placeholder> Signature<T> {
    /// Creates a signature with no parameters and a placeholder return type.
    ///
    /// Use this when constructing IR before type inference has resolved types.
    pub fn placeholder() -> Self {
        Signature {
            params: Vec::new(),
            rets: vec![T::placeholder()],
            constraints: (),
        }
    }
}
