/// Safe left shift that returns `None` when the shift amount is too large
/// (would cause a panic in debug mode or wrapping in release mode).
pub trait CheckedShl: Sized {
    fn checked_shl(self, rhs: Self) -> Option<Self>;
}

/// Safe right shift that returns `None` when the shift amount is too large
/// (would cause a panic in debug mode or wrapping in release mode).
pub trait CheckedShr: Sized {
    fn checked_shr(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_checked_shift_int {
    ($($t:ty),*) => {
        $(
            impl CheckedShl for $t {
                fn checked_shl(self, rhs: Self) -> Option<Self> {
                    let shift: u32 = rhs.try_into().ok()?;
                    <$t>::checked_shl(self, shift)
                }
            }
            impl CheckedShr for $t {
                fn checked_shr(self, rhs: Self) -> Option<Self> {
                    let shift: u32 = rhs.try_into().ok()?;
                    <$t>::checked_shr(self, shift)
                }
            }
        )*
    };
}

impl_checked_shift_int!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

#[cfg(test)]
mod tests {
    use super::{CheckedShl, CheckedShr};

    #[test]
    fn checked_shl_by_zero() {
        assert_eq!(CheckedShl::checked_shl(42i64, 0i64), Some(42));
        assert_eq!(CheckedShl::checked_shl(0i64, 0i64), Some(0));
    }

    #[test]
    fn checked_shl_normal() {
        assert_eq!(CheckedShl::checked_shl(1i64, 3i64), Some(8));
        assert_eq!(CheckedShl::checked_shl(1u64, 10u64), Some(1024));
    }

    #[test]
    fn checked_shl_by_63() {
        assert_eq!(CheckedShl::checked_shl(1i64, 63i64), Some(i64::MIN));
        assert_eq!(CheckedShl::checked_shl(1u64, 63u64), Some(1u64 << 63));
    }

    #[test]
    fn checked_shl_by_64_returns_none() {
        assert_eq!(CheckedShl::checked_shl(1i64, 64i64), None);
        assert_eq!(CheckedShl::checked_shl(1u64, 64u64), None);
    }

    #[test]
    fn checked_shl_negative_shift_returns_none() {
        assert_eq!(CheckedShl::checked_shl(1i64, -1i64), None);
        assert_eq!(CheckedShl::checked_shl(1i64, i64::MIN), None);
    }

    #[test]
    fn checked_shl_max_shift_returns_none() {
        assert_eq!(CheckedShl::checked_shl(1i64, i64::MAX), None);
    }

    #[test]
    fn checked_shr_by_zero() {
        assert_eq!(CheckedShr::checked_shr(42i64, 0i64), Some(42));
        assert_eq!(CheckedShr::checked_shr(-1i64, 0i64), Some(-1));
    }

    #[test]
    fn checked_shr_normal() {
        assert_eq!(CheckedShr::checked_shr(8i64, 3i64), Some(1));
        assert_eq!(CheckedShr::checked_shr(1024u64, 10u64), Some(1));
    }

    #[test]
    fn checked_shr_by_63() {
        assert_eq!(CheckedShr::checked_shr(i64::MIN, 63i64), Some(-1));
        assert_eq!(CheckedShr::checked_shr(u64::MAX, 63u64), Some(1));
    }

    #[test]
    fn checked_shr_by_64_returns_none() {
        assert_eq!(CheckedShr::checked_shr(1i64, 64i64), None);
        assert_eq!(CheckedShr::checked_shr(1u64, 64u64), None);
    }

    #[test]
    fn checked_shr_negative_shift_returns_none() {
        assert_eq!(CheckedShr::checked_shr(1i64, -1i64), None);
        assert_eq!(CheckedShr::checked_shr(1i64, i64::MIN), None);
    }

    #[test]
    fn checked_shr_max_shift_returns_none() {
        assert_eq!(CheckedShr::checked_shr(1i64, i64::MAX), None);
    }
}
