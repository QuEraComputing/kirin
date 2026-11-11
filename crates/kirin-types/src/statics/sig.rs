use super::{Sort, StaticTermId};

/// The type signature of a statement or a callable:
///  `∀ā:σ̄. B̄ ⊃ (T_1, …, T_n) ⇒ U`
///
/// - Each use instantiates the quantifiers `ā` and must prove the guards `B̄`.
/// - There is a single result type `U`, even for statements with multiple
///   results. Several results form one product type, unpacked into the
///   results at each use.
///
/// The quantifiers bind `guards`, `args` and `result` as `Bound` indices:
/// the last quantifier is `Bound(0)` and the first is `Bound(k - 1)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeSig<S> {
    /// The sorts `σ̄` of the quantifiers `ā`.
    pub quantifiers: Vec<Sort<S>>,
    /// The guards `B̄`, each of sort `bool`.
    pub guards: Vec<StaticTermId>,
    /// The argument types `T_1, …, T_n`, each of sort `type`.
    pub args: Vec<StaticTermId>,
    /// The result type `U`, of sort `type`.
    pub result: StaticTermId,
}
