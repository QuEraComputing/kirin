use crate::AbstractValue;

/// Strategy for when to apply widening during fixpoint iteration.
#[derive(Debug, Clone, Copy)]
pub enum WideningStrategy {
    /// Widen at every join point (block with multiple predecessors or loop headers).
    AllJoins,
    /// Only join, never widen. Suitable for finite-height lattices that
    /// guarantee termination without widening.
    Never,
    /// Join for the first `n` visits to each block, then widen.
    Delayed(usize),
}

impl WideningStrategy {
    /// Merge `current` with `incoming` according to this strategy.
    ///
    /// `visit_count` is the number of times the target block has been
    /// revisited (excluding the first visit).
    pub fn merge<V: AbstractValue>(&self, current: &V, incoming: &V, visit_count: usize) -> V {
        match self {
            Self::AllJoins => current.widen(incoming),
            Self::Never => current.join(incoming),
            Self::Delayed(n) => {
                if visit_count <= *n {
                    current.join(incoming)
                } else {
                    current.widen(incoming)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_ir::{HasBottom, Lattice};

    /// A minimal three-element lattice: Bot < Mid < Top.
    /// `join` is the least upper bound, `widen` always jumps to Top.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ThreePoint {
        Bot,
        Mid,
        Top,
    }

    impl Lattice for ThreePoint {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Top, _) | (_, Self::Top) => Self::Top,
                (Self::Mid, _) | (_, Self::Mid) => Self::Mid,
                _ => Self::Bot,
            }
        }

        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Bot, _) | (_, Self::Bot) => Self::Bot,
                (Self::Mid, _) | (_, Self::Mid) => Self::Mid,
                _ => Self::Top,
            }
        }

        fn is_subseteq(&self, other: &Self) -> bool {
            matches!(
                (self, other),
                (Self::Bot, _) | (Self::Mid, Self::Mid | Self::Top) | (Self::Top, Self::Top)
            )
        }
    }

    impl HasBottom for ThreePoint {
        fn bottom() -> Self {
            Self::Bot
        }
    }

    impl AbstractValue for ThreePoint {
        fn widen(&self, _next: &Self) -> Self {
            // Widening always jumps to Top to guarantee termination
            Self::Top
        }
    }

    #[test]
    fn test_widening_strategy_all_joins() {
        let strategy = WideningStrategy::AllJoins;
        let current = ThreePoint::Bot;
        let incoming = ThreePoint::Mid;

        // AllJoins widens at every visit count
        for visit_count in 0..5 {
            let result = strategy.merge(&current, &incoming, visit_count);
            assert_eq!(
                result,
                ThreePoint::Top,
                "AllJoins should widen (-> Top) at visit_count={visit_count}"
            );
        }
    }

    #[test]
    fn test_widening_strategy_never() {
        let strategy = WideningStrategy::Never;
        let current = ThreePoint::Bot;
        let incoming = ThreePoint::Mid;

        // Never should join (not widen) at every visit count
        for visit_count in 0..5 {
            let result = strategy.merge(&current, &incoming, visit_count);
            assert_eq!(
                result,
                ThreePoint::Mid,
                "Never should join (-> Mid) at visit_count={visit_count}"
            );
        }
    }

    #[test]
    fn test_widening_strategy_delayed_threshold() {
        let strategy = WideningStrategy::Delayed(2);
        let current = ThreePoint::Bot;
        let incoming = ThreePoint::Mid;

        // visit_count 0,1,2 -> join (Bot join Mid = Mid)
        for visit_count in 0..=2 {
            let result = strategy.merge(&current, &incoming, visit_count);
            assert_eq!(
                result,
                ThreePoint::Mid,
                "Delayed(2) should join at visit_count={visit_count}"
            );
        }

        // visit_count 3+ -> widen (Bot widen Mid = Top)
        for visit_count in 3..6 {
            let result = strategy.merge(&current, &incoming, visit_count);
            assert_eq!(
                result,
                ThreePoint::Top,
                "Delayed(2) should widen at visit_count={visit_count}"
            );
        }
    }
}
