/// Trait for values that can serve as induction variables in `scf.for` loops.
pub trait ForLoopValue {
    /// Returns whether the loop should continue (`self < end`).
    fn loop_condition(&self, end: &Self) -> Option<bool>;
    /// Advance the induction variable by `step`. Returns `None` on overflow.
    fn loop_step(&self, step: &Self) -> Option<Self>
    where
        Self: Sized;
}

impl ForLoopValue for i64 {
    fn loop_condition(&self, end: &i64) -> Option<bool> {
        Some(*self < *end)
    }

    fn loop_step(&self, step: &i64) -> Option<i64> {
        self.checked_add(*step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_condition_less_than_end() {
        assert_eq!(0i64.loop_condition(&10), Some(true));
    }

    #[test]
    fn loop_condition_equal_to_end() {
        assert_eq!(10i64.loop_condition(&10), Some(false));
    }

    #[test]
    fn loop_condition_greater_than_end() {
        assert_eq!(15i64.loop_condition(&10), Some(false));
    }

    #[test]
    fn loop_condition_negative_range() {
        assert_eq!((-5i64).loop_condition(&-1), Some(true));
        assert_eq!((-1i64).loop_condition(&-5), Some(false));
    }

    #[test]
    fn loop_condition_at_zero() {
        assert_eq!(0i64.loop_condition(&0), Some(false));
        assert_eq!((-1i64).loop_condition(&0), Some(true));
        assert_eq!(0i64.loop_condition(&1), Some(true));
    }

    #[test]
    fn loop_condition_i64_boundaries() {
        assert_eq!(i64::MIN.loop_condition(&i64::MAX), Some(true));
        assert_eq!(i64::MAX.loop_condition(&i64::MIN), Some(false));
        assert_eq!(i64::MAX.loop_condition(&i64::MAX), Some(false));
    }

    #[test]
    fn loop_step_positive() {
        assert_eq!(0i64.loop_step(&1), Some(1));
        assert_eq!(5i64.loop_step(&3), Some(8));
    }

    #[test]
    fn loop_step_negative() {
        assert_eq!(10i64.loop_step(&-1), Some(9));
        assert_eq!(0i64.loop_step(&-5), Some(-5));
    }

    #[test]
    fn loop_step_zero() {
        assert_eq!(42i64.loop_step(&0), Some(42));
    }

    #[test]
    fn loop_step_from_negative() {
        assert_eq!((-10i64).loop_step(&3), Some(-7));
    }

    #[test]
    fn loop_step_overflow_returns_none() {
        assert_eq!(i64::MAX.loop_step(&1), None);
        assert_eq!((i64::MAX - 1).loop_step(&2), None);
    }

    #[test]
    fn loop_step_underflow_returns_none() {
        assert_eq!(i64::MIN.loop_step(&-1), None);
        assert_eq!((i64::MIN + 1).loop_step(&-2), None);
    }

    #[test]
    fn simulate_loop_zero_to_five() {
        let mut iv = 0i64;
        let end = 5i64;
        let step = 1i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step).unwrap();
        }
        assert_eq!(iterations, 5);
        assert_eq!(iv, 5);
    }

    #[test]
    fn simulate_loop_step_two() {
        let mut iv = 0i64;
        let end = 10i64;
        let step = 2i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step).unwrap();
        }
        assert_eq!(iterations, 5);
        assert_eq!(iv, 10);
    }

    #[test]
    fn simulate_loop_empty_range() {
        let iv = 10i64;
        let end = 5i64;
        let mut iterations = 0;
        let mut current = iv;
        while current.loop_condition(&end) == Some(true) {
            iterations += 1;
            current = current.loop_step(&1).unwrap();
        }
        assert_eq!(iterations, 0);
    }

    #[test]
    fn simulate_loop_single_iteration() {
        let mut iv = 0i64;
        let end = 1i64;
        let step = 1i64;
        let mut iterations = 0;
        while iv.loop_condition(&end) == Some(true) {
            iterations += 1;
            iv = iv.loop_step(&step).unwrap();
        }
        assert_eq!(iterations, 1);
    }

    #[test]
    fn loop_condition_always_some() {
        assert!(0i64.loop_condition(&0).is_some());
        assert!(i64::MIN.loop_condition(&i64::MAX).is_some());
        assert!(i64::MAX.loop_condition(&i64::MIN).is_some());
    }
}
