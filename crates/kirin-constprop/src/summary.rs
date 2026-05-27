use kirin_interpreter_new::{FixpointPhase, FunctionEntryTarget, Location, Summary};
use kirin_ir::{CompileStage, HasBottom, HasTop, Lattice, Product};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConstPropFunctionOwner {
    pub stage: CompileStage,
    pub target: FunctionEntryTarget,
}

impl ConstPropFunctionOwner {
    pub fn new(stage: CompileStage, target: FunctionEntryTarget) -> Self {
        Self { stage, target }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstPropOwner {
    Function(ConstPropFunctionOwner),
    Location(Location),
}

impl ConstPropOwner {
    pub fn function(stage: CompileStage, target: FunctionEntryTarget) -> Self {
        Self::Function(ConstPropFunctionOwner::new(stage, target))
    }

    pub fn location(location: Location) -> Self {
        Self::Location(location)
    }

    pub fn stage(&self) -> CompileStage {
        match self {
            Self::Function(owner) => owner.stage,
            Self::Location(location) => location.stage,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstPropFunctionSummary<V> {
    value: V,
}

impl<V> ConstPropFunctionSummary<V> {
    pub fn new(value: V) -> Self {
        Self { value }
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn into_value(self) -> V {
        self.value
    }
}

pub trait ConstPropLocationSummary<V>: Clone + PartialEq {
    fn merge_location(&mut self, candidate: Self) -> bool;
}

impl<V> ConstPropLocationSummary<V> for () {
    fn merge_location(&mut self, _candidate: Self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstPropSummary<V, L = ()> {
    Function(ConstPropFunctionSummary<V>),
    Location(Option<L>),
}

impl<V, L> ConstPropSummary<V, L> {
    pub fn function(value: V) -> Self {
        Self::Function(ConstPropFunctionSummary::new(value))
    }

    pub fn location(state: L) -> Self {
        Self::Location(Some(state))
    }

    pub fn function_bottom() -> Self
    where
        V: HasBottom,
    {
        Self::function(V::bottom())
    }

    pub fn location_bottom() -> Self {
        Self::Location(None)
    }

    pub fn function_value(&self) -> Option<&V> {
        match self {
            Self::Function(summary) => Some(summary.value()),
            Self::Location(_) => None,
        }
    }

    pub fn location_state(&self) -> Option<&L> {
        match self {
            Self::Location(state) => state.as_ref(),
            Self::Function(_) => None,
        }
    }
}

impl<V, L> Summary for ConstPropSummary<V, L>
where
    V: HasBottom + HasTop + Clone + PartialEq,
    L: ConstPropLocationSummary<V>,
{
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        match (self, candidate) {
            (Self::Function(value), Self::Function(candidate)) => {
                merge_value(&mut value.value, &candidate.value)
            }
            (Self::Location(state), Self::Location(Some(candidate))) => {
                merge_location_summary(state, candidate)
            }
            (Self::Location(_), Self::Location(None)) => None,
            _ => None,
        }
    }
}

fn merge_location_summary<V, L>(state: &mut Option<L>, candidate: L) -> Option<()>
where
    L: ConstPropLocationSummary<V>,
{
    let Some(current) = state else {
        *state = Some(candidate);
        return Some(());
    };

    current.merge_location(candidate).then_some(())
}

fn merge_value<V>(value: &mut V, candidate: &V) -> Option<()>
where
    V: Lattice + PartialEq,
{
    let joined = value.join(candidate);
    if *value == joined {
        None
    } else {
        *value = joined;
        Some(())
    }
}

pub fn join_product<V>(current: &Product<V>, candidate: &Product<V>) -> Product<V>
where
    V: HasTop + Clone,
{
    if current.len() != candidate.len() {
        let len = current.len().max(candidate.len());
        return Product::from_vec((0..len).map(|_| V::top()).collect());
    }

    current
        .iter()
        .zip(candidate.iter())
        .map(|(current, candidate)| current.join(candidate))
        .collect()
}

#[cfg(test)]
mod tests {
    use kirin_interpreter_new::{FixpointPhase, Summary};
    use kirin_ir::Product;

    use crate::ConstPropValue;

    use super::{ConstPropLocationSummary, ConstPropSummary, join_product};

    type Value = ConstPropValue<i64, &'static str, &'static str>;

    #[test]
    fn function_summary_joins_return_values() {
        let mut summary: ConstPropSummary<Value> = ConstPropSummary::function(Value::Const(1));
        let mut strategy = ();

        assert_eq!(
            summary.merge(
                FixpointPhase::Widen,
                ConstPropSummary::function(Value::Const(1)),
                &mut strategy,
            ),
            None
        );
        assert_eq!(
            summary.merge(
                FixpointPhase::Widen,
                ConstPropSummary::function(Value::Const(2)),
                &mut strategy,
            ),
            Some(())
        );
        assert_eq!(summary.function_value(), Some(&Value::Top));
    }

    #[test]
    fn location_summary_joins_location_state() {
        let mut summary = ConstPropSummary::location(TestLocationSummary {
            facts: Product::from_vec(vec![Value::Const(1), Value::Const(2)]),
        });
        let mut strategy = ();

        let candidate = ConstPropSummary::location(TestLocationSummary {
            facts: Product::from_vec(vec![Value::Const(1), Value::Const(3)]),
        });

        assert_eq!(
            summary.merge(FixpointPhase::Widen, candidate, &mut strategy),
            Some(())
        );
        let state = summary.location_state().unwrap();
        assert_eq!(
            state.facts,
            Product::from_vec(vec![Value::Const(1), Value::Top])
        );
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestLocationSummary {
        facts: Product<Value>,
    }

    impl ConstPropLocationSummary<Value> for TestLocationSummary {
        fn merge_location(&mut self, candidate: Self) -> bool {
            let joined = join_product(&self.facts, &candidate.facts);
            if self.facts == joined {
                false
            } else {
                self.facts = joined;
                true
            }
        }
    }
}
