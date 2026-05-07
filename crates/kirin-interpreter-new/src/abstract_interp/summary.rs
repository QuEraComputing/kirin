use super::{AbstractEnv, AbstractValue, FixpointPhase, Summary};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidenNarrowStrategy {
    pub widen_after: usize,
    pub narrow_iterations: usize,
}

impl Default for WidenNarrowStrategy {
    fn default() -> Self {
        Self {
            widen_after: 1,
            narrow_iterations: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvSummary<V> {
    env: AbstractEnv<V>,
    pub visits: usize,
    pub narrow_visits: usize,
}

impl<V> Default for EnvSummary<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> EnvSummary<V> {
    pub fn new() -> Self {
        Self {
            env: AbstractEnv::new(),
            visits: 0,
            narrow_visits: 0,
        }
    }

    pub fn from_env(env: AbstractEnv<V>) -> Self {
        Self {
            env,
            visits: 0,
            narrow_visits: 0,
        }
    }

    pub fn env(&self) -> &AbstractEnv<V> {
        &self.env
    }

    pub fn env_mut(&mut self) -> &mut AbstractEnv<V> {
        &mut self.env
    }
}

impl<V> Summary for EnvSummary<V>
where
    V: AbstractValue,
{
    type Strategy = WidenNarrowStrategy;
    type Change = ();

    fn merge(
        &mut self,
        phase: FixpointPhase,
        candidate: Self,
        strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        let use_widening =
            matches!(phase, FixpointPhase::Widen) && self.visits >= strategy.widen_after;
        let mut changed = false;

        for (value, candidate_value) in candidate.env.into_values() {
            let current = self.env.values.entry(value).or_insert_with(V::bottom);
            let merged = match phase {
                FixpointPhase::Join => current.join(&candidate_value),
                FixpointPhase::Widen if use_widening => current.widen(&candidate_value),
                FixpointPhase::Widen => current.join(&candidate_value),
                FixpointPhase::Narrow => current.narrow(&candidate_value),
            };

            if *current != merged {
                *current = merged;
                changed = true;
            }
        }

        match phase {
            FixpointPhase::Narrow => self.narrow_visits += 1,
            _ => self.visits += 1,
        }

        changed.then_some(())
    }
}

#[cfg(test)]
mod tests {
    use kirin_ir::{HasBottom, HasTop, Lattice, TestSSAValue};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TinyValue {
        Bottom,
        Const(i64),
        Top,
    }

    impl Lattice for TinyValue {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
                (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(*lhs),
                _ => Self::Top,
            }
        }

        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Top, value) | (value, Self::Top) => value.clone(),
                (Self::Const(lhs), Self::Const(rhs)) if lhs == rhs => Self::Const(*lhs),
                _ => Self::Bottom,
            }
        }

        fn is_subseteq(&self, other: &Self) -> bool {
            self.join(other) == *other
        }
    }

    impl HasBottom for TinyValue {
        fn bottom() -> Self {
            Self::Bottom
        }
    }

    impl HasTop for TinyValue {
        fn top() -> Self {
            Self::Top
        }
    }

    #[test]
    fn env_summary_joins_candidate_values() {
        let value = TestSSAValue(0).into();
        let mut summary = EnvSummary::new();
        let mut strategy = WidenNarrowStrategy::default();
        let mut first = AbstractEnv::new();
        first.write(value, TinyValue::Const(1));

        assert_eq!(
            summary.merge(
                FixpointPhase::Join,
                EnvSummary::from_env(first),
                &mut strategy
            ),
            Some(())
        );
        assert_eq!(summary.env().read(value), TinyValue::Const(1));

        let mut second = AbstractEnv::new();
        second.write(value, TinyValue::Const(2));
        summary.merge(
            FixpointPhase::Join,
            EnvSummary::from_env(second),
            &mut strategy,
        );

        assert_eq!(summary.env().read(value), TinyValue::Top);
    }
}
