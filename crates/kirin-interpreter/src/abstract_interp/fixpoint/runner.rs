use crate::{FixpointProfile, Frame, FrameEffect, InterpreterError, StepResult};

use super::StandardFixpointInterpreter;

impl<'ir, P, Store, Deps> StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
{
    pub fn run_frame(&mut self, root: P::Frame) -> Result<P::Completion, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        if !self.frame_stack.is_empty() {
            return Err(P::Error::from(InterpreterError::Custom(
                "cannot start a frame run with a non-empty frame stack",
            )));
        }
        self.frame_stack.push(root);

        loop {
            let frame = match self.frame_stack.pop() {
                Some(frame) => frame,
                None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
            };
            let effect = frame.step(self)?;
            if let StepResult::Complete(completion) = self.apply_local_effect(effect)? {
                return Ok(completion);
            }
        }
    }

    fn apply_local_effect(
        &mut self,
        mut effect: FrameEffect<P::Frame, P::Completion>,
    ) -> Result<StepResult<P::Completion>, P::Error>
    where
        P::Frame: Frame<Self, P::Frame, P::Completion, P::Error>,
        P::Error: From<InterpreterError>,
    {
        loop {
            match effect {
                FrameEffect::Continue(frame) => {
                    self.frame_stack.push(frame);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Push { parent, child } => {
                    self.frame_stack.push(parent);
                    self.frame_stack.push(child);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Done => {
                    let parent = match self.frame_stack.pop() {
                        Some(parent) => parent,
                        None => return Err(P::Error::from(InterpreterError::EmptyFrameStack)),
                    };
                    effect = parent.resume_done(self)?;
                }
                FrameEffect::Complete(completion) => {
                    if let Some(parent) = self.frame_stack.pop() {
                        effect = parent.resume(completion, self)?;
                    } else {
                        return Ok(StepResult::Complete(completion));
                    }
                }
            }
        }
    }
}
