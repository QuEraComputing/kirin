use kirin_ir::{HasBottom, HasTop, Lattice};

// ============================================================================
// Interval Domain
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    NegInf,
    Finite(i64),
    PosInf,
}

impl Bound {
    pub fn min(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, _) | (_, Bound::NegInf) => Bound::NegInf,
            (Bound::PosInf, b) | (b, Bound::PosInf) => b,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.min(b)),
        }
    }

    pub fn max(self, other: Self) -> Self {
        match (self, other) {
            (Bound::PosInf, _) | (_, Bound::PosInf) => Bound::PosInf,
            (Bound::NegInf, b) | (b, Bound::NegInf) => b,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.max(b)),
        }
    }

    pub fn less_than(self, other: Self) -> bool {
        match (self, other) {
            (Bound::NegInf, Bound::NegInf) => false,
            (Bound::NegInf, _) => true,
            (_, Bound::NegInf) => false,
            (Bound::PosInf, _) => false,
            (_, Bound::PosInf) => true,
            (Bound::Finite(a), Bound::Finite(b)) => a < b,
        }
    }

    pub fn less_eq(self, other: Self) -> bool {
        self == other || self.less_than(other)
    }

    pub fn saturating_add(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, Bound::PosInf) | (Bound::PosInf, Bound::NegInf) => Bound::NegInf,
            (Bound::NegInf, _) | (_, Bound::NegInf) => Bound::NegInf,
            (Bound::PosInf, _) | (_, Bound::PosInf) => Bound::PosInf,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_add(b)),
        }
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        match (self, other) {
            (Bound::NegInf, Bound::NegInf) | (Bound::PosInf, Bound::PosInf) => Bound::NegInf,
            (Bound::NegInf, _) | (_, Bound::PosInf) => Bound::NegInf,
            (Bound::PosInf, _) | (_, Bound::NegInf) => Bound::PosInf,
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_sub(b)),
        }
    }

    pub fn saturating_mul(self, other: Self) -> Self {
        match (self, other) {
            (Bound::Finite(0), _) | (_, Bound::Finite(0)) => Bound::Finite(0),
            (Bound::NegInf, Bound::NegInf) | (Bound::PosInf, Bound::PosInf) => Bound::PosInf,
            (Bound::NegInf, Bound::PosInf) | (Bound::PosInf, Bound::NegInf) => Bound::NegInf,
            (Bound::NegInf, Bound::Finite(b)) | (Bound::Finite(b), Bound::NegInf) => {
                if b > 0 {
                    Bound::NegInf
                } else {
                    Bound::PosInf
                }
            }
            (Bound::PosInf, Bound::Finite(b)) | (Bound::Finite(b), Bound::PosInf) => {
                if b > 0 {
                    Bound::PosInf
                } else {
                    Bound::NegInf
                }
            }
            (Bound::Finite(a), Bound::Finite(b)) => Bound::Finite(a.saturating_mul(b)),
        }
    }

    pub fn negate(self) -> Self {
        match self {
            Bound::NegInf => Bound::PosInf,
            Bound::PosInf => Bound::NegInf,
            Bound::Finite(v) => Bound::Finite(-v),
        }
    }
}

/// An interval [lo, hi] where lo > hi represents bottom (empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub lo: Bound,
    pub hi: Bound,
}

impl Interval {
    pub fn new(lo: i64, hi: i64) -> Self {
        if lo > hi {
            Self::bottom_interval()
        } else {
            Interval {
                lo: Bound::Finite(lo),
                hi: Bound::Finite(hi),
            }
        }
    }

    pub fn constant(v: i64) -> Self {
        Interval::new(v, v)
    }

    pub fn bottom_interval() -> Self {
        Interval {
            lo: Bound::PosInf,
            hi: Bound::NegInf,
        }
    }

    pub fn half_bounded_above(hi: i64) -> Self {
        Interval {
            lo: Bound::NegInf,
            hi: Bound::Finite(hi),
        }
    }

    pub fn half_bounded_below(lo: i64) -> Self {
        Interval {
            lo: Bound::Finite(lo),
            hi: Bound::PosInf,
        }
    }

    pub fn is_empty(&self) -> bool {
        match (self.lo, self.hi) {
            (Bound::PosInf, _) => true,
            (_, Bound::NegInf) => true,
            (Bound::Finite(lo), Bound::Finite(hi)) => lo > hi,
            (Bound::NegInf, _) => false,
            (_, Bound::PosInf) => false,
        }
    }
}

// ============================================================================
// Interval Arithmetic
// ============================================================================

pub fn interval_add(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.lo.saturating_add(b.lo),
        hi: a.hi.saturating_add(b.hi),
    }
}

pub fn interval_sub(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.lo.saturating_sub(b.hi),
        hi: a.hi.saturating_sub(b.lo),
    }
}

pub fn interval_mul(a: &Interval, b: &Interval) -> Interval {
    if a.is_empty() || b.is_empty() {
        return Interval::bottom_interval();
    }
    let products = [
        a.lo.saturating_mul(b.lo),
        a.lo.saturating_mul(b.hi),
        a.hi.saturating_mul(b.lo),
        a.hi.saturating_mul(b.hi),
    ];
    let lo = products.iter().copied().fold(Bound::PosInf, Bound::min);
    let hi = products.iter().copied().fold(Bound::NegInf, Bound::max);
    Interval { lo, hi }
}

pub fn interval_neg(a: &Interval) -> Interval {
    if a.is_empty() {
        return Interval::bottom_interval();
    }
    Interval {
        lo: a.hi.negate(),
        hi: a.lo.negate(),
    }
}

// ============================================================================
// Lattice + AbstractValue impls
// ============================================================================

impl Lattice for Interval {
    fn join(&self, other: &Self) -> Self {
        if self.is_empty() {
            return other.clone();
        }
        if other.is_empty() {
            return self.clone();
        }
        Interval {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    fn meet(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        if lo.less_eq(hi) {
            Interval { lo, hi }
        } else {
            Interval::bottom_interval()
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        if self.is_empty() {
            return true;
        }
        if other.is_empty() {
            return false;
        }
        other.lo.less_eq(self.lo) && self.hi.less_eq(other.hi)
    }
}

impl HasBottom for Interval {
    fn bottom() -> Self {
        Interval::bottom_interval()
    }
}

impl HasTop for Interval {
    fn top() -> Self {
        Interval {
            lo: Bound::NegInf,
            hi: Bound::PosInf,
        }
    }
}

impl std::ops::Add for Interval {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        interval_add(&self, &rhs)
    }
}

impl std::ops::Sub for Interval {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        interval_sub(&self, &rhs)
    }
}

impl std::ops::Mul for Interval {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        interval_mul(&self, &rhs)
    }
}

impl std::ops::Div for Interval {
    type Output = Self;
    fn div(self, _rhs: Self) -> Self {
        Interval::top()
    }
}

impl std::ops::Rem for Interval {
    type Output = Self;
    fn rem(self, _rhs: Self) -> Self {
        Interval::top()
    }
}

impl std::ops::Neg for Interval {
    type Output = Self;
    fn neg(self) -> Self {
        interval_neg(&self)
    }
}

#[cfg(feature = "interpreter")]
impl kirin_interpreter::BranchCondition for Interval {
    fn is_truthy(&self) -> Option<bool> {
        if self.is_empty() {
            return None;
        }
        let all_negative = match self.hi {
            Bound::NegInf => true,
            Bound::Finite(h) => h < 0,
            Bound::PosInf => false,
        };
        let all_positive = match self.lo {
            Bound::PosInf => true,
            Bound::Finite(l) => l > 0,
            Bound::NegInf => false,
        };
        if all_negative || all_positive {
            return Some(true);
        }
        if *self == Interval::constant(0) {
            return Some(false);
        }
        None
    }
}

#[cfg(feature = "cmp")]
impl kirin_cmp::CompareValue for Interval {
    fn cmp_eq(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        // Both are single points and equal → definitely 1
        if self == other && self.lo == self.hi {
            return Interval::constant(1);
        }
        // Ranges are disjoint → definitely 0
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_ne(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if self == other && self.lo == self.hi {
            return Interval::constant(0);
        }
        if self.hi.less_than(other.lo) || other.hi.less_than(self.lo) {
            return Interval::constant(1);
        }
        Interval::new(0, 1)
    }

    fn cmp_lt(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        // self.hi < other.lo → definitely true
        if self.hi.less_than(other.lo) {
            return Interval::constant(1);
        }
        // other.hi <= self.lo → definitely false
        if other.hi.less_eq(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_le(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if self.hi.less_eq(other.lo) {
            return Interval::constant(1);
        }
        if other.hi.less_than(self.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_gt(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if other.hi.less_than(self.lo) {
            return Interval::constant(1);
        }
        if self.hi.less_eq(other.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }

    fn cmp_ge(&self, other: &Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Interval::bottom_interval();
        }
        if other.hi.less_eq(self.lo) {
            return Interval::constant(1);
        }
        if self.hi.less_than(other.lo) {
            return Interval::constant(0);
        }
        Interval::new(0, 1)
    }
}

#[cfg(feature = "arith")]
impl From<kirin_arith::ArithValue> for Interval {
    fn from(v: kirin_arith::ArithValue) -> Self {
        use kirin_arith::ArithValue;
        match v {
            ArithValue::I64(x) => Interval::constant(x),
            ArithValue::I32(x) => Interval::constant(x as i64),
            ArithValue::I16(x) => Interval::constant(x as i64),
            ArithValue::I8(x) => Interval::constant(x as i64),
            _ => Interval::top(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::lattice::assert_finite_lattice_laws;

    #[test]
    fn interval_lattice_laws() {
        let elements = vec![
            Interval::bottom(),
            Interval::constant(0),
            Interval::constant(42),
            Interval::new(0, 10),
            Interval::new(-5, 5),
            Interval::new(3, 7),
            Interval::new(-100, 100),
            Interval::top(),
        ];
        assert_finite_lattice_laws(&elements);
    }

    #[test]
    fn test_interval_lattice_basic() {
        let a = Interval::new(0, 10);
        let b = Interval::new(5, 20);

        assert_eq!(a.join(&b), Interval::new(0, 20));
        assert_eq!(a.meet(&b), Interval::new(5, 10));
        assert!(Interval::new(2, 8).is_subseteq(&a));
        assert!(!b.is_subseteq(&a));

        let bot = <Interval as HasBottom>::bottom();
        assert!(bot.is_empty());
        assert!(bot.is_subseteq(&a));
        assert_eq!(bot.join(&a), a);
        assert_eq!(a.meet(&bot), bot);

        let top = Interval::top();
        assert!(a.is_subseteq(&top));
        assert_eq!(a.join(&top), top);
        assert_eq!(a.meet(&top), a);
    }

    #[test]
    fn test_interval_arithmetic() {
        let a = Interval::new(1, 5);
        let b = Interval::new(10, 20);

        assert_eq!(interval_add(&a, &b), Interval::new(11, 25));
        assert_eq!(interval_sub(&b, &a), Interval::new(5, 19));
        assert_eq!(Interval::constant(42), Interval::new(42, 42));
    }

    #[test]
    fn test_interval_arithmetic_with_infinity() {
        let a = Interval::half_bounded_below(0);
        let b = Interval::constant(1);
        let sum = interval_add(&a, &b);
        assert_eq!(sum.lo, Bound::Finite(1));
        assert_eq!(sum.hi, Bound::PosInf);
    }

    fn representative_intervals() -> Vec<Interval> {
        vec![
            Interval::bottom_interval(),
            Interval::top(),
            Interval::constant(0),
            Interval::constant(42),
            Interval::constant(-10),
            Interval::new(0, 100),
            Interval::new(-50, 50),
            Interval::new(1, 1),
            Interval::half_bounded_below(0),
            Interval::half_bounded_above(100),
            Interval::new(-1000, 1000),
        ]
    }

    #[test]
    fn test_join_associativity() {
        for a in &representative_intervals() {
            for b in &representative_intervals() {
                for c in &representative_intervals() {
                    assert_eq!(a.join(b).join(c), a.join(&b.join(c)));
                }
            }
        }
    }

    #[test]
    fn test_join_commutativity() {
        for a in &representative_intervals() {
            for b in &representative_intervals() {
                assert_eq!(a.join(b), b.join(a));
            }
        }
    }

    #[test]
    fn test_bottom_is_identity_for_join() {
        let bot = <Interval as HasBottom>::bottom();
        for a in &representative_intervals() {
            assert_eq!(a.join(&bot), *a);
            assert_eq!(bot.join(a), *a);
        }
    }
}

#[cfg(all(test, feature = "interpreter"))]
mod widen_narrow_tests {
    use super::*;
    use kirin_interpreter::AbstractValue;

    fn representative_intervals() -> Vec<Interval> {
        vec![
            Interval::bottom_interval(),
            Interval::top(),
            Interval::constant(0),
            Interval::constant(42),
            Interval::constant(-10),
            Interval::new(0, 100),
            Interval::new(-50, 50),
            Interval::new(1, 1),
            Interval::half_bounded_below(0),
            Interval::half_bounded_above(100),
            Interval::new(-1000, 1000),
        ]
    }

    #[test]
    fn test_widen_basic() {
        let a = Interval::new(0, 5);
        let b = Interval::new(0, 10);
        let w = a.widen(&b);
        assert_eq!(w.lo, Bound::Finite(0));
        assert_eq!(w.hi, Bound::PosInf);

        let c = Interval::new(-5, 5);
        let w = a.widen(&c);
        assert_eq!(w.lo, Bound::NegInf);
        assert_eq!(w.hi, Bound::Finite(5));
    }

    #[test]
    fn test_narrow_basic() {
        let wide = Interval {
            lo: Bound::Finite(0),
            hi: Bound::PosInf,
        };
        let refined = Interval::new(0, 100);
        assert_eq!(wide.narrow(&refined), Interval::new(0, 100));

        let wide2 = Interval::top();
        assert_eq!(
            wide2.narrow(&Interval::new(-50, 50)),
            Interval::new(-50, 50)
        );
    }

    /// Simulates: `x = 0; while (x < 100) { x = x + 1 }; return x`
    #[test]
    fn test_manual_fixpoint_widening_narrowing() {
        let one = Interval::constant(1);

        // Iteration 1: x = [0,0]
        let mut x = Interval::constant(0);

        // Iteration 2: loop body: x+1 = [1,1], join with entry: [0,1]
        let x_next = interval_add(&x, &one);
        let x_joined = x.join(&x_next);
        assert_eq!(x_joined, Interval::new(0, 1));

        // Widen: lo stable (0), hi grew (0→1) => push hi to +inf
        x = x.widen(&x_joined);
        assert_eq!(x.lo, Bound::Finite(0));
        assert_eq!(x.hi, Bound::PosInf);

        // Iteration 3: [0,+inf) + [1,1] = [1,+inf), join => [0,+inf)
        let x_next2 = interval_add(&x, &one);
        let x_joined2 = x.join(&x_next2);
        let x_widened = x.widen(&x_joined2);
        assert_eq!(x_widened, x); // fixpoint

        // Narrowing: apply constraint x <= 100
        let loop_constraint = Interval::new(0, 100);
        let x_narrowed = x.narrow(&loop_constraint);
        assert_eq!(x_narrowed, Interval::new(0, 100));
    }

    #[test]
    fn test_widen_monotonicity() {
        for x in &representative_intervals() {
            for y in &representative_intervals() {
                let w = x.widen(y);
                assert!(x.is_subseteq(&w), "x={x:?} not subseteq widen(x,y)={w:?}");
                assert!(y.is_subseteq(&w), "y={y:?} not subseteq widen(x,y)={w:?}");
            }
        }
    }

    #[test]
    fn test_narrow_bounds() {
        for x in &representative_intervals() {
            for y in &representative_intervals() {
                let n = x.narrow(y);
                let m = x.meet(y);
                assert!(
                    m.is_subseteq(&n),
                    "meet not subseteq narrow for x={x:?}, y={y:?}"
                );
                assert!(
                    n.is_subseteq(x),
                    "narrow not subseteq x for x={x:?}, y={y:?}"
                );
            }
        }
    }
}

#[cfg(feature = "interpreter")]
impl kirin_interpreter::AbstractValue for Interval {
    fn widen(&self, next: &Self) -> Self {
        if self.is_empty() {
            return next.clone();
        }
        if next.is_empty() {
            return self.clone();
        }
        let lo = if next.lo.less_than(self.lo) {
            Bound::NegInf
        } else {
            self.lo
        };
        let hi = if self.hi.less_than(next.hi) {
            Bound::PosInf
        } else {
            self.hi
        };
        Interval { lo, hi }
    }

    fn narrow(&self, next: &Self) -> Self {
        if self.is_empty() || next.is_empty() {
            return self.clone();
        }
        let lo = match self.lo {
            Bound::NegInf => next.lo,
            other => other,
        };
        let hi = match self.hi {
            Bound::PosInf => next.hi,
            other => other,
        };
        Interval { lo, hi }
    }
}
