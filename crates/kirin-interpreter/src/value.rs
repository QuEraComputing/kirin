use kirin_ir::HasBottom;

/// Decidability of a branch condition.
///
/// Interpreter value types implement this to support conditional branching
/// in [`kirin_cf::ControlFlow::ConditionalBranch`].
///
/// - `Some(true)` — definitely truthy, take the true branch.
/// - `Some(false)` — definitely falsy, take the false branch.
/// - `None` — undecidable (abstract interpreters should fork).
pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}

/// Abstract value extending [`Lattice`] with widening and narrowing.
///
/// No blanket implementation — every abstract value type must explicitly
/// define its own widening operator.
///
/// ## Algebraic contracts
///
/// **Widening**: `x ⊑ widen(x, y)` and `y ⊑ widen(x, y)`. The ascending
/// chain `x₀, widen(x₀, x₁), widen(widen(x₀, x₁), x₂), ...` must stabilize
/// in finite steps.
///
/// **Narrowing**: `x ⊓ y ⊑ narrow(x, y) ⊑ x`. The descending chain must
/// also stabilize in finite steps.
pub trait AbstractValue: HasBottom {
    /// Widen `self` with `next` to guarantee ascending chain termination.
    fn widen(&self, next: &Self) -> Self;

    /// Narrow `self` with `next` to refine a post-fixpoint downward.
    ///
    /// Default: no refinement (returns `self`).
    fn narrow(&self, _next: &Self) -> Self
    where
        Self: Clone,
    {
        self.clone()
    }
}
