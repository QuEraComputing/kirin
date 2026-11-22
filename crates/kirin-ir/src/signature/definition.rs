use crate::Product;

/// A function signature parameterized over the type `T` and optional constraints `C`.
///
/// - `params`: the parameter types, one per argument.
/// - `rets`: the return types, one per returned value.
/// - `constraints`: optional constraint context (e.g., type-variable bindings). Defaults to `()`.
///
/// Both sides are a [`Product`], the framework's container for the values
/// crossing a boundary: call arguments on the way in, `ret` operands and call
/// results on the way out. A `Product` is a sequence of types, one per slot,
/// and is never the type of a single SSA value, so `T` needs no tuple type to
/// express multiple returns. A language that has tuples defines its own tuple
/// type (see [`HasProduct`](crate::HasProduct) and `kirin-tuple`). Arity is exact:
/// `-> A, B` (two returns) and `-> Tuple[A, B]` (one return whose type is the
/// language's own tuple) are different signatures and never convert into each
/// other, so a language that models multiple returns as an explicit tuple
/// keeps a single return.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Signature<T, C = ()> {
    params: Product<T>,
    rets: Product<T>,
    constraints: C,
}

impl<T, C> Signature<T, C> {
    /// Creates a new signature with a single return type.
    pub fn new(params: impl IntoIterator<Item = T>, ret: T, constraints: C) -> Self {
        Self::with_rets(params, [ret], constraints)
    }

    /// Creates a new signature with the given return types (empty means no returns).
    pub fn with_rets(
        params: impl IntoIterator<Item = T>,
        rets: impl IntoIterator<Item = T>,
        constraints: C,
    ) -> Self {
        Signature {
            params: params.into_iter().collect(),
            rets: rets.into_iter().collect(),
            constraints,
        }
    }

    /// Returns the parameter types of the signature.
    pub fn params(&self) -> &Product<T> {
        &self.params
    }

    /// Returns the return types of the signature, one per returned value.
    pub fn rets(&self) -> &Product<T> {
        &self.rets
    }

    /// Returns the constraints of the signature.
    pub fn constraints(&self) -> &C {
        &self.constraints
    }

    /// Formats the return types as a comma-separated list: `T`, `T, U`, or
    /// the empty string for no returns.
    pub fn rets_to_string(&self) -> String
    where
        T: std::fmt::Display,
    {
        let rets: Vec<String> = self.rets.iter().map(ToString::to_string).collect();
        rets.join(", ")
    }
}

/// Prints `(T, U) -> V, W`; a signature with no returns prints `(T, U)`.
impl<T: std::fmt::Display, C> std::fmt::Display for Signature<T, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(")?;
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", param)?;
        }
        write!(f, ")")?;
        if !self.rets.is_empty() {
            write!(f, " -> {}", self.rets_to_string())?;
        }
        Ok(())
    }
}

impl<T: crate::Placeholder> Signature<T> {
    /// Creates a signature with no parameters and one placeholder return type.
    ///
    /// Use this when constructing IR before type inference has resolved types.
    ///
    /// This is a default dispatch key for a function declared without a
    /// signature, not a claim about its arity: the parameter list is empty in
    /// the same way the return list has one entry. Whether a definition
    /// declares a signature at all is answered by
    /// [`HasSignature::signature`](crate::HasSignature::signature) returning
    /// `None`, so a checker must not compare a body against this value.
    pub fn placeholder() -> Self {
        Signature {
            params: Product::new(),
            rets: crate::product![T::placeholder()],
            constraints: (),
        }
    }
}
