use kirin::prelude::{Block, CompileTimeValue, Dialect, HasBottom, HasTop, Lattice, Product};
use kirin_constprop::{
    ConstPropLocationSummary, ConstPropOwner, ConstPropSummary, ConstPropValue, join_product,
};
use kirin_interpreter_new::BlockTransfer;

use crate::ForLoopValue;
use crate::interpreter_new::{ForContinuation, ScfForFixpointSummary};

impl<S, F> ForLoopValue for ConstPropValue<i64, S, F>
where
    S: Clone,
    F: Clone,
{
    fn loop_condition(&self, end: &Self) -> Option<bool> {
        match (self, end) {
            (Self::Const(lhs), Self::Const(rhs)) => Some(lhs < rhs),
            _ => None,
        }
    }

    fn loop_step(&self, step: &Self) -> Option<Self> {
        match (self, step) {
            (Self::Const(lhs), Self::Const(rhs)) => lhs.checked_add(*rhs).map(Self::Const),
            (Self::Bottom, _) | (_, Self::Bottom) => Some(Self::Bottom),
            _ => Some(Self::Top),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScfForConstPropSummary<V> {
    pub body: Block,
    pub init_arg_count: usize,
    pub iv: V,
    pub end: V,
    pub step: V,
    pub carried: Product<V>,
}

impl<V> ScfForConstPropSummary<V> {
    pub fn body_args(&self) -> Product<V>
    where
        V: Clone,
    {
        let mut args = Vec::with_capacity(1 + self.init_arg_count);
        args.push(self.iv.clone());
        args.extend(self.carried.iter().take(self.init_arg_count).cloned());
        Product::from_vec(args)
    }

    pub fn advance_with(
        self,
        carried: Product<V>,
        step: impl FnOnce(&V, &V) -> Option<V>,
    ) -> Option<Self> {
        let iv = step(&self.iv, &self.step)?;
        Some(Self {
            iv,
            carried,
            ..self
        })
    }
}

impl<V> ConstPropLocationSummary<V> for ScfForConstPropSummary<V>
where
    V: HasBottom + HasTop + Clone + PartialEq,
{
    fn merge_location(&mut self, candidate: Self) -> bool {
        let mut changed = false;
        changed |= merge_value(&mut self.iv, &candidate.iv);
        changed |= merge_value(&mut self.end, &candidate.end);
        changed |= merge_value(&mut self.step, &candidate.step);

        let joined_carried = join_product(&self.carried, &candidate.carried);
        if self.carried != joined_carried {
            self.carried = joined_carried;
            changed = true;
        }

        changed
    }
}

impl<L, T, V, X> ScfForFixpointSummary<L, T, V, X, ConstPropOwner>
    for ConstPropSummary<V, ScfForConstPropSummary<V>>
where
    L: Dialect,
    T: CompileTimeValue,
    V: HasBottom + HasTop + Clone + PartialEq,
    X: BlockTransfer<Value = V>,
{
    fn scf_for_owner(continuation: &ForContinuation<L, T, V, X>) -> Option<ConstPropOwner> {
        Some(ConstPropOwner::location(continuation.location))
    }

    fn scf_for_initial_summary(continuation: &ForContinuation<L, T, V, X>) -> Self {
        Self::location(ScfForConstPropSummary {
            body: continuation.body,
            init_arg_count: continuation.init_args.len(),
            iv: continuation.iv.clone(),
            end: continuation.end.clone(),
            step: continuation.step.clone(),
            carried: continuation.carried.clone(),
        })
    }

    fn scf_for_results(&self) -> Option<Product<V>> {
        self.location_state().map(|state| state.carried.clone())
    }
}

fn merge_value<V>(value: &mut V, candidate: &V) -> bool
where
    V: Lattice + PartialEq,
{
    let joined = value.join(candidate);
    if *value == joined {
        false
    } else {
        *value = joined;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Value = ConstPropValue<i64, &'static str, &'static str>;

    #[test]
    fn constprop_value_supports_scf_loop_semantics() {
        assert_eq!(Value::Const(0).loop_condition(&Value::Const(2)), Some(true));
        assert_eq!(
            Value::Const(0).loop_step(&Value::Const(2)),
            Some(Value::Const(2))
        );
        assert_eq!(Value::Top.loop_condition(&Value::Const(2)), None);
    }
}
