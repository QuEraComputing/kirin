use kirin_ir::Lattice;

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
pub trait AbstractValue: Lattice {
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
