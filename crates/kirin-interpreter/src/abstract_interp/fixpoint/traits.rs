#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixpointPhase {
    Join,
    Widen,
    Narrow,
}

pub trait Summary: Clone {
    type Strategy;
    type Change;

    fn merge(
        &mut self,
        phase: FixpointPhase,
        candidate: Self,
        strategy: &mut Self::Strategy,
    ) -> Option<Self::Change>;
}

pub trait OwnerSemantics<I, K, S, F, C, E>
where
    S: Summary,
{
    fn bottom_summary(&mut self, interp: &mut I, owner: &K) -> Result<S, E>;

    fn entry_frame(&mut self, interp: &mut I, owner: &K, summary: &S) -> Result<F, E>;

    fn complete_owner(
        &mut self,
        interp: &mut I,
        owner: K,
        completion: C,
    ) -> Result<SummaryEffect<K, S>, E>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryEffect<K, S> {
    None,
    Update { owner: K, candidate: S },
    Many(Vec<(K, S)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkItem<K> {
    Analyze(K),
}
