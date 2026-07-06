//! Owner reanalysis until the summary stops changing.

use crate::{Frame, FrameEffect, FrameEngine, InterpreterError};

use super::super::*;
use super::support::UnitInterp;

struct CounterProfile;

impl FixpointProfile<UnitInterp> for CounterProfile {
    type SummaryKey = u8;
    type Summary = CounterSummary;
    type Frame = CounterFrame;
    type Completion = u8;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CounterSummary(u8);

impl Summary for CounterSummary {
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
struct CounterFrame(u8);

impl<D: FrameEngine<Error = InterpreterError>> Frame<D> for CounterFrame {
    type Completion = u8;

    fn step(self, _interp: &mut D) -> Result<FrameEffect<Self, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(self.0.saturating_add(1).min(2)))
    }

    fn resume_done(self, _interp: &mut D) -> Result<FrameEffect<Self, u8>, InterpreterError> {
        Ok(FrameEffect::Done)
    }

    fn resume(
        self,
        completion: u8,
        _interp: &mut D,
    ) -> Result<FrameEffect<Self, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(completion))
    }
}

struct CounterSemantics;

type CounterInterp =
    StandardFixpointInterpreter<UnitInterp, CounterProfile, (), OwnerSummaryDeps<u8>>;

impl OwnerSemantics<CounterInterp, u8, CounterSummary, CounterFrame, u8, InterpreterError>
    for CounterSemantics
{
    fn bottom_summary(
        &mut self,
        _interp: &mut CounterInterp,
        _owner: &u8,
    ) -> Result<CounterSummary, InterpreterError> {
        Ok(CounterSummary(0))
    }

    fn entry_frame(
        &mut self,
        _interp: &mut CounterInterp,
        _owner: &u8,
        summary: &CounterSummary,
    ) -> Result<CounterFrame, InterpreterError> {
        Ok(CounterFrame(summary.0))
    }

    fn complete_owner(
        &mut self,
        _interp: &mut CounterInterp,
        owner: u8,
        completion: u8,
    ) -> Result<SummaryEffect<u8, CounterSummary>, InterpreterError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: CounterSummary(completion),
        })
    }
}

#[test]
fn simple_fixpoint_reanalyzes_until_summary_stops_changing() {
    let mut interp = CounterInterp::new(UnitInterp, (), ());
    let mut semantics = CounterSemantics;

    interp.solve(&mut semantics, 0).unwrap();

    assert_eq!(interp.summary(&0), Some(&CounterSummary(2)));
}
