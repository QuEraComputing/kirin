/// Why stage dispatch returned no action result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageDispatchMiss {
    /// The requested stage ID is not present in the pipeline.
    MissingStage,
    /// The stage exists but no dialect in `S::Languages` matched it.
    MissingDialect,
}

/// Error for required dispatch helpers.
#[derive(Debug, PartialEq, Eq)]
pub enum StageDispatchRequiredError<E> {
    /// Action-specific failure produced by `StageAction`/`StageActionMut`.
    Action(E),
    /// Dispatch miss describing why no stage action could run.
    Miss(StageDispatchMiss),
}
