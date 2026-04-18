use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::core::Core;
use crate::env::Env;
use crate::lift::{Lift, Project};

/// Shared interface for both concrete and abstract interpreters.
///
/// Extracts the stage-info lookup and function-resolution machinery from
/// [`ConcreteDomain`] so that dialect ops (particularly `kirin-cf` and
/// `kirin-function`) compile against either execution mode without duplication.
///
/// `ConcreteDomain` extends this with cursor-stack semantics
/// (`take_pending_yield`). `AbstractInterp` implements only `BaseDomain`.
///
/// [`ConcreteDomain`]: crate::concrete::ConcreteDomain
pub trait BaseDomain: Env + Sized
where
    Self::Effect: Lift<Core<Self::Value, Self::Cursor>> + Project<Core<Self::Value, Self::Cursor>>,
{
    /// The containing language for this interpreter.
    ///
    /// Dialect `Interpretable<E>` impls reference `E::Language` instead of a
    /// free `L: Dialect` type parameter to avoid E0207.
    type Language: Dialect;

    /// The cursor type pushed via `Core::Push`.
    ///
    /// For `AbstractInterp` this is `()` — abstract execution does not use
    /// cursor stacks; the field exists so that `Core<V, E::Cursor>` is
    /// well-typed in dialect impls.
    type Cursor;

    type StageContainer: StageMeta;

    fn stage_info_for<L: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<L>>
    where
        Self::StageContainer: HasStageInfo<L>;

    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, Self::Error>;
}
