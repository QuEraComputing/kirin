use std::hash::Hash;

use kirin_interpreter_new::{
    AbstractEnvStore, AbstractValue, Env, HasProductValue, OwnerSummaryDeps,
    StandardFixpointInterpreter, Summary,
};
use kirin_ir::LiftFrom;

use crate::ConstPropValue;

pub type ConstPropFixpointInterpreter<
    'ir,
    Stage,
    K,
    F,
    C,
    E,
    S,
    Store = AbstractEnvStore<ConstPropValue>,
    Deps = OwnerSummaryDeps<K>,
> = StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>;

pub trait ConstPropDomain: AbstractValue + HasProductValue {}

impl<T> ConstPropDomain for T where T: AbstractValue + HasProductValue {}

pub trait ConstPropInterpreterShell<V>: Env<V> {}

impl<'ir, Stage, K, F, C, E, S, Store, Deps, V> ConstPropInterpreterShell<V>
    for StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>
where
    K: Clone + Eq + Hash,
    S: Summary,
    Store: Env<V>,
    E: LiftFrom<Store::Error>,
    V: ConstPropDomain,
{
}

#[cfg(test)]
mod tests {
    use kirin_interpreter_new::{EnvSummary, InterpreterError};

    use super::*;

    #[test]
    fn default_alias_satisfies_constprop_shell_marker() {
        fn assert_shell<I>()
        where
            I: ConstPropInterpreterShell<ConstPropValue, Error = InterpreterError>,
        {
        }

        assert_shell::<
            ConstPropFixpointInterpreter<
                'static,
                (),
                u8,
                (),
                (),
                InterpreterError,
                EnvSummary<ConstPropValue>,
            >,
        >();
    }
}
