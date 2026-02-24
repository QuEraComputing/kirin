use crate::comptime::CompileTimeValue;

/// A lattice with join (least upper bound), meet (greatest lower bound),
/// and a subset ordering.
///
/// Implementations must satisfy the following algebraic laws:
///
/// **Join** (least upper bound):
/// - Associative: `a.join(&b).join(&c) == a.join(&b.join(&c))`
/// - Commutative: `a.join(&b) == b.join(&a)`
/// - Idempotent: `a.join(&a) == a`
///
/// **Meet** (greatest lower bound):
/// - Associative: `a.meet(&b).meet(&c) == a.meet(&b.meet(&c))`
/// - Commutative: `a.meet(&b) == b.meet(&a)`
/// - Idempotent: `a.meet(&a) == a`
///
/// **Absorption**:
/// - `a.join(&a.meet(&b)) == a`
/// - `a.meet(&a.join(&b)) == a`
///
/// **Ordering consistency** (`is_subseteq` must agree with join/meet):
/// - `a.is_subseteq(&b)` if and only if `a.join(&b) == b`
/// - `a.is_subseteq(&b)` if and only if `a.meet(&b) == a`
///
/// Use the assertion helpers in `kirin-test-utils` to verify these properties
/// in tests.
pub trait Lattice {
    fn join(&self, other: &Self) -> Self;
    fn meet(&self, other: &Self) -> Self;
    fn is_subseteq(&self, other: &Self) -> bool;
}

/// A lattice with a bottom element (least element).
///
/// The bottom element must satisfy:
/// - `bottom().is_subseteq(&x)` for all `x`
/// - `bottom().join(&x) == x` for all `x`
/// - `bottom().meet(&x) == bottom()` for all `x`
pub trait HasBottom: Lattice {
    fn bottom() -> Self;
}

/// A lattice with a top element (greatest element).
///
/// The top element must satisfy:
/// - `x.is_subseteq(&top())` for all `x`
/// - `top().join(&x) == top()` for all `x`
/// - `top().meet(&x) == x` for all `x`
pub trait HasTop: Lattice {
    fn top() -> Self;
}

/// A lattice that has both a bottom and a top element.
pub trait FiniteLattice: HasBottom + HasTop {}

impl<T: HasBottom + HasTop> FiniteLattice for T {}

pub trait TypeLattice: FiniteLattice + CompileTimeValue + Default {}
