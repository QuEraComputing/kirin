use kirin_ir::Pipeline;

use crate::{
    FixpointProfile, Frame, FrameEffect, HasLocation, InterpreterError, InterpreterProfile,
    Location,
};

use super::super::*;

struct PhaseProfile;

impl InterpreterProfile for PhaseProfile {
    type Stage = ();
    type Value = ();
    type Frame = PhaseFrame;
    type Completion = u8;
    type Error = InterpreterError;
}

impl FixpointProfile for PhaseProfile {
    type SummaryKey = u8;
    type Summary = PhaseSummary;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PhaseSummary(u8);

impl Summary for PhaseSummary {
    type Strategy = ();
    type Change = ();

    fn merge(
        &mut self,
        phase: FixpointPhase,
        candidate: Self,
        _strategy: &mut Self::Strategy,
    ) -> Option<Self::Change> {
        let next = match phase {
            FixpointPhase::Join | FixpointPhase::Widen if candidate.0 > self.0 => candidate.0,
            FixpointPhase::Narrow if candidate.0 < self.0 => candidate.0,
            _ => return None,
        };
        self.0 = next;
        Some(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhaseFrame;

impl HasLocation for PhaseFrame {
    fn location(&self) -> Location {
        panic!("phase test frame has no IR location")
    }
}

type PhaseInterp<'ir> = StandardFixpointInterpreter<'ir, PhaseProfile, (), OwnerSummaryDeps<u8>>;

impl<'ir> Frame<PhaseInterp<'ir>, PhaseFrame, u8, InterpreterError> for PhaseFrame {
    fn step(
        self,
        interp: &mut PhaseInterp<'ir>,
    ) -> Result<FrameEffect<PhaseFrame, u8>, InterpreterError> {
        let completion = match interp.phase() {
            FixpointPhase::Join => 1,
            FixpointPhase::Widen => 10,
            FixpointPhase::Narrow => 3,
        };
        Ok(FrameEffect::Complete(completion))
    }

    fn resume_done(
        self,
        _interp: &mut PhaseInterp<'ir>,
    ) -> Result<FrameEffect<PhaseFrame, u8>, InterpreterError> {
        Ok(FrameEffect::Done)
    }

    fn resume(
        self,
        completion: u8,
        _interp: &mut PhaseInterp<'ir>,
    ) -> Result<FrameEffect<PhaseFrame, u8>, InterpreterError> {
        Ok(FrameEffect::Complete(completion))
    }
}

struct PhaseSemantics;

impl<'ir> OwnerSemantics<PhaseInterp<'ir>, u8, PhaseSummary, PhaseFrame, u8, InterpreterError>
    for PhaseSemantics
{
    fn bottom_summary(
        &mut self,
        _interp: &mut PhaseInterp<'ir>,
        _owner: &u8,
    ) -> Result<PhaseSummary, InterpreterError> {
        Ok(PhaseSummary(0))
    }

    fn entry_frame(
        &mut self,
        _interp: &mut PhaseInterp<'ir>,
        _owner: &u8,
        _summary: &PhaseSummary,
    ) -> Result<PhaseFrame, InterpreterError> {
        Ok(PhaseFrame)
    }

    fn complete_owner(
        &mut self,
        _interp: &mut PhaseInterp<'ir>,
        owner: u8,
        completion: u8,
    ) -> Result<SummaryEffect<u8, PhaseSummary>, InterpreterError> {
        Ok(SummaryEffect::Update {
            owner,
            candidate: PhaseSummary(completion),
        })
    }
}

#[test]
fn narrowing_revisits_summaries_after_widening() {
    let pipeline = Pipeline::new();
    let mut interp = PhaseInterp::new(&pipeline, (), ());
    let mut semantics = PhaseSemantics;

    interp.solve(&mut semantics, 0).unwrap();
    assert_eq!(interp.summary(&0), Some(&PhaseSummary(10)));

    interp.run_narrowing(&mut semantics, 1).unwrap();
    assert_eq!(interp.summary(&0), Some(&PhaseSummary(3)));
}
