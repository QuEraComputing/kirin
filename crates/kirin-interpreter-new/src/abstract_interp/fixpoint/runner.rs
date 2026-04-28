use std::hash::Hash;

use crate::{Frame, FrameEffect, InterpreterError, StepResult};

use super::{SimpleFixpointInterpreter, Summary};

impl<'ir, Stage, K, F, C, E, S, Store> SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn run_frame(&mut self, root: F) -> Result<C, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        let mut stack = vec![root];

        loop {
            let frame = stack.pop().ok_or(InterpreterError::EmptyFrameStack)?;
            let effect = frame.step(self)?;
            if let StepResult::Complete(completion) = self.apply_local_effect(&mut stack, effect)? {
                return Ok(completion);
            }
        }
    }

    fn apply_local_effect(
        &mut self,
        stack: &mut Vec<F>,
        effect: FrameEffect<F, C>,
    ) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        match effect {
            FrameEffect::Continue(frame) => {
                stack.push(frame);
                Ok(StepResult::Running)
            }
            FrameEffect::Push { parent, child } => {
                stack.push(parent);
                stack.push(child);
                Ok(StepResult::Running)
            }
            FrameEffect::Done => {
                if let Some(parent) = stack.pop() {
                    let effect = parent.resume_done(self)?;
                    self.apply_local_effect(stack, effect)
                } else {
                    Err(InterpreterError::EmptyFrameStack.into())
                }
            }
            FrameEffect::Complete(completion) => {
                if let Some(parent) = stack.pop() {
                    let effect = parent.resume(completion, self)?;
                    self.apply_local_effect(stack, effect)
                } else {
                    Ok(StepResult::Complete(completion))
                }
            }
        }
    }
}
