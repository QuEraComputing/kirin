use kirin_interpreter_new::AbstractEnvStore;

use crate::{ConstPropFixpointInterpreter, ConstPropOwner, ConstPropSummary, ConstPropValue};

/// Standard const-prop function fixpoint alias.
///
/// Pins the [`ConstPropFixpointInterpreter`] to use [`ConstPropOwner`] as
/// the owner type, [`ConstPropSummary`] as the summary type, and the
/// default [`AbstractEnvStore`] for the SSA store.
pub type ConstPropFunctionFixpoint<'ir, Stage, F, C, E, V = ConstPropValue, L = ()> =
    ConstPropFixpointInterpreter<
        'ir,
        Stage,
        ConstPropOwner,
        F,
        C,
        E,
        ConstPropSummary<V, L>,
        AbstractEnvStore<V>,
    >;
