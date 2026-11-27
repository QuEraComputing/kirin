use super::{Sort, StaticTermId};

/// The type signature of a statement, block or a callable, in prenex form:
///  `∀ā:σ̄. B̄ ⊃ T̄ ⇒ ∃b̄:ρ̄. C̄ ∧ Ū`
///  No argument or result contains a `∀`, `∃`, `⊃` or `∧` anywhere: those go
/// in the fields above.
///
/// - Each use instantiates the quantifiers `ā` and must prove the guards `B̄`.
/// - Each use opens the existentials `b̄` with fresh rigid variables and may
///   assume the asserts `C̄`.
/// - `args` and `results` have one entry per operand or result field of the
///   statement. An entry has sort `type` for a single value, or `[type]` for
///   a list of values (e.g. a `Vec<SSAValue>` field). A quantifier of list
///   sort, e.g. `∀τ̄:[type]`, lets one signature serve any number of values.
///
/// Binders, as [`Bound`](crate::StaticTerm::Bound) indices (the innermost
/// binder is `Bound(0)`):
/// - `guards` and `args` are under the quantifiers: the last quantifier is
///   `Bound(0)` and the first is `Bound(k - 1)`;
/// - `asserts` and `results` are under the quantifiers and then the
///   existentials, so the last existential is `Bound(0)`. Instantiate them
///   with the quantifiers' arguments followed by the existentials' witnesses.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeSig<S> {
    /// The sorts `σ̄` of the quantifiers `ā`.
    pub quantifiers: Vec<Sort<S>>,
    /// The guards `B̄`, each of sort `bool`.
    pub guards: Vec<StaticTermId>,
    /// The argument types `T̄`, each of sort `type` or `[type]`.
    pub args: Vec<StaticTermId>,
    /// The sorts `ρ̄` of the existentials `b̄`.
    pub existentials: Vec<Sort<S>>,
    /// The asserts `C̄`, each of sort `bool`.
    pub asserts: Vec<StaticTermId>,
    /// The result types `Ū`, each of sort `type` or `[type]`.
    pub results: Vec<StaticTermId>,
}
