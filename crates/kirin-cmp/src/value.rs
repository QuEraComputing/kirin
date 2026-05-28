/// Comparison operations on values.
///
/// The `Bool` associated type is the result domain for comparisons.
/// For concrete interpreters this is typically the same as `Self` (e.g. `i64`),
/// while abstract interpreters may use a dedicated boolean domain.
pub trait CompareValue {
    type Bool;

    fn cmp_eq(&self, other: &Self) -> Self::Bool;
    fn cmp_ne(&self, other: &Self) -> Self::Bool;
    fn cmp_lt(&self, other: &Self) -> Self::Bool;
    fn cmp_le(&self, other: &Self) -> Self::Bool;
    fn cmp_gt(&self, other: &Self) -> Self::Bool;
    fn cmp_ge(&self, other: &Self) -> Self::Bool;
}

impl CompareValue for i64 {
    type Bool = i64;

    fn cmp_eq(&self, other: &Self) -> Self::Bool {
        if self == other { 1 } else { 0 }
    }
    fn cmp_ne(&self, other: &Self) -> Self::Bool {
        if self != other { 1 } else { 0 }
    }
    fn cmp_lt(&self, other: &Self) -> Self::Bool {
        if self < other { 1 } else { 0 }
    }
    fn cmp_le(&self, other: &Self) -> Self::Bool {
        if self <= other { 1 } else { 0 }
    }
    fn cmp_gt(&self, other: &Self) -> Self::Bool {
        if self > other { 1 } else { 0 }
    }
    fn cmp_ge(&self, other: &Self) -> Self::Bool {
        if self >= other { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_equal_values() {
        assert_eq!(42i64.cmp_eq(&42), 1);
    }

    #[test]
    fn eq_different_values() {
        assert_eq!(1i64.cmp_eq(&2), 0);
    }

    #[test]
    fn ne_equal_values() {
        assert_eq!(42i64.cmp_ne(&42), 0);
    }

    #[test]
    fn ne_different_values() {
        assert_eq!(1i64.cmp_ne(&2), 1);
    }

    #[test]
    fn lt_less() {
        assert_eq!(1i64.cmp_lt(&2), 1);
    }

    #[test]
    fn lt_equal() {
        assert_eq!(2i64.cmp_lt(&2), 0);
    }

    #[test]
    fn lt_greater() {
        assert_eq!(3i64.cmp_lt(&2), 0);
    }

    #[test]
    fn le_less() {
        assert_eq!(1i64.cmp_le(&2), 1);
    }

    #[test]
    fn le_equal() {
        assert_eq!(2i64.cmp_le(&2), 1);
    }

    #[test]
    fn le_greater() {
        assert_eq!(3i64.cmp_le(&2), 0);
    }

    #[test]
    fn gt_greater() {
        assert_eq!(3i64.cmp_gt(&2), 1);
    }

    #[test]
    fn gt_equal() {
        assert_eq!(2i64.cmp_gt(&2), 0);
    }

    #[test]
    fn gt_less() {
        assert_eq!(1i64.cmp_gt(&2), 0);
    }

    #[test]
    fn ge_greater() {
        assert_eq!(3i64.cmp_ge(&2), 1);
    }

    #[test]
    fn ge_equal() {
        assert_eq!(2i64.cmp_ge(&2), 1);
    }

    #[test]
    fn ge_less() {
        assert_eq!(1i64.cmp_ge(&2), 0);
    }

    #[test]
    fn eq_zero() {
        assert_eq!(0i64.cmp_eq(&0), 1);
    }

    #[test]
    fn lt_negative_values() {
        assert_eq!((-5i64).cmp_lt(&-3), 1);
        assert_eq!((-3i64).cmp_lt(&-5), 0);
    }

    #[test]
    fn cmp_i64_min_max() {
        assert_eq!(i64::MIN.cmp_lt(&i64::MAX), 1);
        assert_eq!(i64::MAX.cmp_gt(&i64::MIN), 1);
        assert_eq!(i64::MIN.cmp_eq(&i64::MIN), 1);
        assert_eq!(i64::MAX.cmp_eq(&i64::MAX), 1);
        assert_eq!(i64::MIN.cmp_ne(&i64::MAX), 1);
    }

    #[test]
    fn le_ge_at_boundaries() {
        assert_eq!(i64::MIN.cmp_le(&i64::MIN), 1);
        assert_eq!(i64::MAX.cmp_ge(&i64::MAX), 1);
        assert_eq!(i64::MIN.cmp_le(&i64::MAX), 1);
        assert_eq!(i64::MAX.cmp_ge(&i64::MIN), 1);
    }

    #[test]
    fn cmp_negative_zero() {
        assert_eq!(0i64.cmp_eq(&(-0i64)), 1);
    }

    #[test]
    fn results_are_boolean_ints() {
        let pairs: &[(i64, i64)] = &[(0, 0), (1, 2), (-1, 1), (i64::MIN, i64::MAX)];
        for &(a, b) in pairs {
            for result in [
                a.cmp_eq(&b),
                a.cmp_ne(&b),
                a.cmp_lt(&b),
                a.cmp_le(&b),
                a.cmp_gt(&b),
                a.cmp_ge(&b),
            ] {
                assert!(
                    result == 0 || result == 1,
                    "expected 0 or 1, got {result} for ({a}, {b})"
                );
            }
        }
    }

    #[test]
    fn eq_ne_complementary() {
        let pairs: &[(i64, i64)] = &[(0, 0), (1, 2), (-1, -1), (i64::MIN, i64::MAX)];
        for &(a, b) in pairs {
            assert_eq!(
                a.cmp_eq(&b) + a.cmp_ne(&b),
                1,
                "eq + ne should be 1 for ({a}, {b})"
            );
        }
    }

    #[test]
    fn lt_ge_complementary() {
        let pairs: &[(i64, i64)] = &[(0, 0), (1, 2), (2, 1), (i64::MIN, i64::MAX)];
        for &(a, b) in pairs {
            assert_eq!(
                a.cmp_lt(&b) + a.cmp_ge(&b),
                1,
                "lt + ge should be 1 for ({a}, {b})"
            );
        }
    }

    #[test]
    fn gt_le_complementary() {
        let pairs: &[(i64, i64)] = &[(0, 0), (1, 2), (2, 1), (i64::MIN, i64::MAX)];
        for &(a, b) in pairs {
            assert_eq!(
                a.cmp_gt(&b) + a.cmp_le(&b),
                1,
                "gt + le should be 1 for ({a}, {b})"
            );
        }
    }
}
