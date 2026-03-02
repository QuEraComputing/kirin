use crate::context::StageInfo;
use crate::language::Dialect;
use crate::node::function::CompileStage;
use crate::stage::meta::{HasStageInfo, StageMeta};

/// Immutable stage action executed after resolving a concrete stage dialect.
///
/// Implement this trait for each dialect in your stage container's
/// `S::Languages` type tuple.
pub trait StageAction<S, L>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output;
    type Error;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &StageInfo<L>,
    ) -> Result<Self::Output, Self::Error>;
}

/// Mutable stage action executed after resolving a concrete stage dialect.
///
/// Implement this trait for each dialect in your stage container's
/// `S::Languages` type tuple.
pub trait StageActionMut<S, L>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
{
    type Output;
    type Error;

    fn run(
        &mut self,
        stage_id: CompileStage,
        stage: &mut StageInfo<L>,
    ) -> Result<Self::Output, Self::Error>;
}
