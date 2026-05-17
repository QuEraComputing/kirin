use kirin_ir::Pipeline;

use crate::{Frame, FrameEffect, HasLocation, InterpreterError, Location};

use super::super::*;

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

impl HasLocation for CounterFrame {
    fn location(&self) -> Location {
        panic!("counter test frame has no IR location")
    }
}

impl<I> Frame<I, CounterFrame, u8, InterpreterError> for CounterFrame {
    fn step(self, _interp: &mut I) -> Result<FrameEffect<CounterFrame, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(self.0.saturating_add(1).min(2)))
    }

    fn resume_done(
        self,
        _interp: &mut I,
    ) -> Result<FrameEffect<CounterFrame, u8>, InterpreterError> {
        Ok(FrameEffect::Done)
    }

    fn resume(
        self,
        completion: u8,
        _interp: &mut I,
    ) -> Result<FrameEffect<CounterFrame, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(completion))
    }
}

struct CounterSemantics;

type CounterInterp<'ir> = StandardFixpointInterpreter<
    'ir,
    (),
    u8,
    CounterFrame,
    u8,
    InterpreterError,
    CounterSummary,
    (),
    OwnerSummaryDeps<u8>,
>;

impl<'ir> OwnerSemantics<CounterInterp<'ir>, u8, CounterSummary, CounterFrame, u8, InterpreterError>
    for CounterSemantics
{
    fn bottom_summary(
        &mut self,
        _interp: &mut CounterInterp<'ir>,
        _owner: &u8,
    ) -> Result<CounterSummary, InterpreterError> {
        Ok(CounterSummary(0))
    }

    fn entry_frame(
        &mut self,
        _interp: &mut CounterInterp<'ir>,
        _owner: &u8,
        summary: &CounterSummary,
    ) -> Result<CounterFrame, InterpreterError> {
        Ok(CounterFrame(summary.0))
    }

    fn complete_owner(
        &mut self,
        _interp: &mut CounterInterp<'ir>,
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
    let pipeline = Pipeline::new();
    let mut interp = CounterInterp::new(&pipeline, (), ());
    let mut semantics = CounterSemantics;

    interp.solve(&mut semantics, 0).unwrap();

    assert_eq!(interp.summary(&0), Some(&CounterSummary(2)));
}
