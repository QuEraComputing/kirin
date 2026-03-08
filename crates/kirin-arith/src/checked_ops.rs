/// Safe division that returns `None` on division by zero instead of panicking.
pub trait CheckedDiv: Sized {
    fn checked_div(self, rhs: Self) -> Option<Self>;
}

/// Safe remainder that returns `None` on division by zero instead of panicking.
pub trait CheckedRem: Sized {
    fn checked_rem(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_checked_int {
    ($($t:ty),*) => {
        $(
            impl CheckedDiv for $t {
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_div(self, rhs)
                }
            }
            impl CheckedRem for $t {
                fn checked_rem(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_rem(self, rhs)
                }
            }
        )*
    };
}

impl_checked_int!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

macro_rules! impl_checked_float {
    ($($t:ty),*) => {
        $(
            impl CheckedDiv for $t {
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    // Float division by zero produces infinity/NaN, not a panic.
                    Some(self / rhs)
                }
            }
            impl CheckedRem for $t {
                fn checked_rem(self, rhs: Self) -> Option<Self> {
                    // Float remainder by zero produces NaN, not a panic.
                    Some(self % rhs)
                }
            }
        )*
    };
}

impl_checked_float!(f32, f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_div_by_zero_returns_none() {
        assert_eq!(10i64.checked_div(0), None);
        assert_eq!(10i32.checked_div(0), None);
        assert_eq!(10u64.checked_div(0), None);
    }

    #[test]
    fn checked_div_normal() {
        assert_eq!(10i64.checked_div(2), Some(5));
        assert_eq!(10i32.checked_div(3), Some(3));
        assert_eq!(10u64.checked_div(5), Some(2));
    }

    #[test]
    fn checked_div_i64_min_by_neg_one() {
        assert_eq!(i64::MIN.checked_div(-1), None);
    }

    #[test]
    fn checked_rem_by_zero_returns_none() {
        assert_eq!(10i64.checked_rem(0), None);
        assert_eq!(10u32.checked_rem(0), None);
    }

    #[test]
    fn checked_rem_normal() {
        assert_eq!(10i64.checked_rem(3), Some(1));
        assert_eq!(10u64.checked_rem(4), Some(2));
    }

    #[test]
    fn checked_rem_i64_min_by_neg_one() {
        assert_eq!(i64::MIN.checked_rem(-1), None);
    }

    #[test]
    fn float_checked_div_by_zero_returns_some() {
        let result = 1.0f64.checked_div(0.0);
        assert!(result.is_some());
        assert!(result.unwrap().is_infinite());
    }

    #[test]
    fn float_checked_rem_by_zero_returns_some() {
        let result = 1.0f64.checked_rem(0.0);
        assert!(result.is_some());
        assert!(result.unwrap().is_nan());
    }
}
