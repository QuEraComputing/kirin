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

    /// Saturating division of two bounds.
    ///
    /// The divisor must not be `Finite(0)` — callers must exclude zero-spanning
    /// divisors before calling this. Inf/Inf cases should not arise when the
    /// divisor does not span zero; we conservatively handle them anyway.
    pub fn saturating_div(self, other: Self) -> Self {
        match (self, other) {
            (Bound::Finite(a), Bound::Finite(b)) => {
                debug_assert!(b != 0, "saturating_div called with zero divisor");
                Bound::Finite(a / b)
            }
            (Bound::PosInf, Bound::Finite(b)) => {
                if b > 0 {
                    Bound::PosInf
                } else {
                    Bound::NegInf
                }
            }
            (Bound::NegInf, Bound::Finite(b)) => {
                if b > 0 {
                    Bound::NegInf
                } else {
                    Bound::PosInf
                }
            }
            // Finite / Inf — anything finite divided by infinity truncates to 0
            (Bound::Finite(_), Bound::PosInf | Bound::NegInf) => Bound::Finite(0),
            // Inf / Inf — conservative fallback (same sign → PosInf, opposite → NegInf)
            (Bound::PosInf, Bound::PosInf) | (Bound::NegInf, Bound::NegInf) => Bound::PosInf,
            (Bound::PosInf, Bound::NegInf) | (Bound::NegInf, Bound::PosInf) => Bound::NegInf,
        }
    }

    /// Negate this bound. For `Finite(i64::MIN)`, `checked_neg` fails and we
    /// return `PosInf` as a sound over-approximation since `|i64::MIN| > i64::MAX`.
    pub fn negate(self) -> Self {
        match self {
            Bound::NegInf => Bound::PosInf,
            Bound::PosInf => Bound::NegInf,
            Bound::Finite(v) => match v.checked_neg() {
                Some(neg) => Bound::Finite(neg),
                None => Bound::PosInf,
            },
        }
    }
}
