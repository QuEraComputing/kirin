use kirin_ir::{
    CompileStage, Dialect, HasStageInfo, SpecializedFunction, StageInfo, StageMeta, Symbol,
};

use crate::store::Store;

/// Base interpreter trait shared by both `ConcreteInterp` and `AbstractInterp`.
///
/// Dialect authors implement `Interpretable<E>` for their op types, constrained
/// on `E: Interp`.
///
/// # Key differences from interpreter-6's `BaseDomain`
///
/// - No `Lift<Control> + Project<Control>` in the supertrait. Those bounds appear
///   only in `Execute::execute` (concrete) and `AbstractInterp::run_block`
///   (abstract), not here.
/// - `type Language` renamed to `type Dialect` for clarity.
/// - `type Ext` replaces the implicit cursor in `Core<V,C>`: `ControlExt<C>`
///   for concrete, `Infallible` for abstract.
pub trait Interp: Store {
    /// The dialect this interpreter is configured for.
    ///
    /// SCF cursor types are parameterized by this — e.g. `IfCursor<V, E::Dialect>`.
    type Dialect: Dialect;

    /// Extension effect type for this interpreter mode.
    ///
    /// - `ConcreteInterp`: `ControlExt<C>` where `C` is the cursor coproduct.
    /// - `AbstractInterp`: `Infallible` (proves at the type level that no cursor
    ///   events occur during abstract execution).
    type Ext;

    /// The stage container type for the pipeline.
    type StageContainer: StageMeta;

    /// The stage ID currently being executed.
    fn current_stage(&self) -> CompileStage;

    /// Look up the `StageInfo<L>` for a given stage ID.
    fn stage_info_for<L: Dialect>(&self, stage_id: CompileStage) -> Option<&StageInfo<L>>
    where
        Self::StageContainer: HasStageInfo<L>;

    /// Resolve a function symbol to a `SpecializedFunction`.
    ///
    /// Uses the interpreter's home dialect (`Self::Dialect`) for stage-info lookup.
    /// The where clause must be added at call sites where `Self::StageContainer:
    /// HasStageInfo<Self::Dialect>` is not in scope.
    fn resolve_function(
        &self,
        target: Symbol,
        stage_id: CompileStage,
    ) -> Result<SpecializedFunction, Self::Error>
    where
        Self::StageContainer: HasStageInfo<Self::Dialect>;
}
