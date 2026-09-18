//! Forward/backward dependency scheduling, and no implicit same-owner reanalysis.

use crate::{Frame, FrameEffect, FrameEngine, InterpreterError};

use super::super::*;
use super::support::UnitInterp;

struct DepProfile;

impl FixpointProfile<UnitInterp> for DepProfile {
    type SummaryKey = u8;
    type Summary = DepSummary;
    type Frame = DepFrame;
    type Completion = u8;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DepSummary(u8);

impl Summary for DepSummary {
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        _phase: FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        if candidate.0 > self.0 {
            self.0 = candidate.0;
            Some(())
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DepFrame {
    owner: u8,
}

impl<D: FrameEngine<Error = InterpreterError>, F> Frame<D, F> for DepFrame {
    type Completion = u8;

    fn step_into(self, _interp: &mut D) -> Result<FrameEffect<F, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(self.owner.saturating_add(1)))
    }

    fn resume_done_into(self, _interp: &mut D) -> Result<FrameEffect<F, u8>, InterpreterError> {
        Ok(FrameEffect::Done)
    }

    fn resume_into(
        self,
        completion: u8,
        _interp: &mut D,
    ) -> Result<FrameEffect<F, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(completion))
    }
}

struct DepSemantics;

type DepInterp<Deps> = StandardFixpointInterpreter<UnitInterp, DepProfile, (), Deps>;

impl<Deps> OwnerSemantics<DepInterp<Deps>, u8, DepSummary, DepFrame, u8, InterpreterError>
    for DepSemantics
where
    Deps: SummaryDependencyIndex<u8>,
{
    fn bottom_summary(
        &mut self,
        _interp: &mut DepInterp<Deps>,
        _owner: &u8,
    ) -> Result<DepSummary, InterpreterError> {
        Ok(DepSummary(0))
    }

    fn entry_frame(
        &mut self,
        _interp: &mut DepInterp<Deps>,
        owner: &u8,
        _summary: &DepSummary,
    ) -> Result<DepFrame, InterpreterError> {
        Ok(DepFrame { owner: *owner })
    }

    fn complete_owner(
        &mut self,
        _interp: &mut DepInterp<Deps>,
        owner: u8,
        completion: u8,
    ) -> Result<SummaryEffect<u8, DepSummary>, InterpreterError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: DepSummary(completion),
        })
    }
}

#[test]
fn forward_dependencies_schedule_successors_when_summary_changes() {
    let mut deps = ForwardSummaryDeps::new();
    deps.register(&0, SummaryDependency::Reanalyze(1)).unwrap();
    let mut interp = DepInterp::with_dependency_index(UnitInterp, (), (), deps);
    let mut semantics = DepSemantics;

    interp.solve(&mut semantics, 0).unwrap();

    assert_eq!(interp.summary(&0), Some(&DepSummary(1)));
    assert_eq!(interp.summary(&1), Some(&DepSummary(2)));
}

#[test]
fn forward_dependencies_do_not_implicitly_reanalyze_the_same_owner() {
    let mut interp =
        DepInterp::with_dependency_index(UnitInterp, (), (), ForwardSummaryDeps::new());
    let mut semantics = DepSemantics;

    interp.solve(&mut semantics, 0).unwrap();

    assert_eq!(interp.summary(&0), Some(&DepSummary(1)));
}

#[test]
fn backward_dependencies_schedule_predecessors_when_summary_changes() {
    let mut deps = BackwardSummaryDeps::new();
    deps.register(&1, SummaryDependency::Reanalyze(0)).unwrap();
    let mut interp = DepInterp::with_dependency_index(UnitInterp, (), (), deps);
    let mut semantics = DepSemantics;

    interp.solve(&mut semantics, 1).unwrap();

    assert_eq!(interp.summary(&1), Some(&DepSummary(2)));
    assert_eq!(interp.summary(&0), Some(&DepSummary(1)));
}

#[test]
fn backward_dependencies_do_not_implicitly_reanalyze_the_same_owner() {
    let mut interp =
        DepInterp::with_dependency_index(UnitInterp, (), (), BackwardSummaryDeps::new());
    let mut semantics = DepSemantics;

    interp.solve(&mut semantics, 1).unwrap();

    assert_eq!(interp.summary(&1), Some(&DepSummary(2)));
}
