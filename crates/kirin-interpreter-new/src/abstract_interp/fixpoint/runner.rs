use std::hash::Hash;

use crate::{Frame, FrameEffect, InterpreterError, StepResult};

use super::{StandardFixpointInterpreter, Summary};

impl<'ir, Stage, K, F, C, E, S, Store, Deps>
    StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>
where
    K: Clone + Eq + Hash,
    S: Summary,
{
    pub fn run_frame(&mut self, root: F) -> Result<C, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
    {
        if !self.frame_stack.is_empty() {
            return Err(E::from(InterpreterError::Custom(
                "cannot start a frame run with a non-empty frame stack",
            )));
        }
        self.frame_stack.push(root);

        loop {
            let frame = match self.frame_stack.pop() {
                Some(frame) => frame,
                None => return Err(E::from(InterpreterError::EmptyFrameStack)),
            };
            let effect = frame.step(self)?;
            if let StepResult::Complete(completion) = self.apply_local_effect(effect)? {
                return Ok(completion);
            }
        }
    }

    fn apply_local_effect(&mut self, mut effect: FrameEffect<F, C>) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: From<InterpreterError>,
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
                        None => return Err(E::from(InterpreterError::EmptyFrameStack)),
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
