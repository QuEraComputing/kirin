use crate::context::StageInfo;
use crate::language::Dialect;
use crate::node::function::CompileStage;
use crate::node::symbol::GlobalSymbol;

/// Trait for types that contain a [`StageInfo`] for a specific dialect.
///
/// Parameterized by dialect type `L` so that enums with multiple stage variants
/// can implement it multiple times -- once per dialect:
///
/// ```ignore
/// enum Stage {
///     A(StageInfo<LangA>),
///     B(StageInfo<LangB>),
/// }
///
/// impl HasStageInfo<LangA> for Stage {
///     fn try_stage_info(&self) -> Option<&StageInfo<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
///     fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<LangA>> {
///         match self { Stage::A(ctx) => Some(ctx), _ => None }
///     }
/// }
/// ```
///
/// Composable via bounds: `S: HasStageInfo<LangA> + HasStageInfo<LangB>`.
pub trait HasStageInfo<L: Dialect> {
    /// Try to get a reference to the stage info for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain stage info for dialect `L`
    /// (e.g., an enum variant for a different dialect).
    fn try_stage_info(&self) -> Option<&StageInfo<L>>;

    /// Try to get a mutable reference to the stage info for dialect `L`.
    ///
    /// Returns `None` if this stage does not contain stage info for dialect `L`.
    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<L>>;
}

// Base case: StageInfo<L> trivially provides stage info for L.
impl<L: Dialect> HasStageInfo<L> for StageInfo<L> {
    fn try_stage_info(&self) -> Option<&StageInfo<L>> {
        Some(self)
    }

    fn try_stage_info_mut(&mut self) -> Option<&mut StageInfo<L>> {
        Some(self)
    }
}

/// Unified trait for stage identity and stage-container metadata.
///
/// Automatically implemented for [`StageInfo<L>`]. Allows [`Pipeline::add_stage`]
/// to set both a readable name and the numeric stage ID on the stage.
///
/// For enum stages, derive this trait with `#[derive(StageMeta)]`:
///
/// ```ignore
/// #[derive(StageMeta)]
/// enum Stage {
///     #[stage(name = "A")]
///     Parse(StageInfo<LangA>),
///     #[stage(name = "B")]
///     Lower(StageInfo<LangB>),
/// }
/// ```
pub trait StageMeta: Sized {
    /// The dialect dispatch list for `pipeline.parse(text)`.
    ///
    /// For heterogeneous pipelines use nested tuples: `(LangA, (LangB, ()))`.
    type Languages;

    /// Get the stage name, if set.
    fn stage_name(&self) -> Option<GlobalSymbol>;
    /// Set the stage name.
    fn set_stage_name(&mut self, name: Option<GlobalSymbol>);
    /// Get the compile-stage ID, if set.
    fn stage_id(&self) -> Option<CompileStage>;
    /// Set the compile-stage ID.
    fn set_stage_id(&mut self, id: Option<CompileStage>);

    /// Build a concrete stage from a parsed stage name (`@...`).
    fn from_stage_name(stage_name: &str) -> Result<Self, String>;

    /// The set of stage names this container recognizes (for typo suggestions).
    fn declared_stage_names() -> &'static [&'static str] {
        &[]
    }
}

impl<L: Dialect> StageMeta for StageInfo<L> {
    type Languages = (L, ());

    fn stage_name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    fn set_stage_name(&mut self, name: Option<GlobalSymbol>) {
        self.name = name;
    }

    fn stage_id(&self) -> Option<CompileStage> {
        self.stage_id
    }

    fn set_stage_id(&mut self, id: Option<CompileStage>) {
        self.stage_id = id;
    }

    fn from_stage_name(_stage_name: &str) -> Result<Self, String> {
        Ok(StageInfo::default())
    }
}
