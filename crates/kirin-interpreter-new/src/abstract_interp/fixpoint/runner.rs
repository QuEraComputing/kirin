use std::hash::Hash;

use kirin_ir::LiftFrom;

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
        E: LiftFrom<InterpreterError>,
    {
        let mut stack = vec![root];

        loop {
            let frame = match stack.pop() {
                Some(frame) => frame,
                None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
            };
            let effect = frame.step(self)?;
            if let StepResult::Complete(completion) = self.apply_local_effect(&mut stack, effect)? {
                return Ok(completion);
            }
        }
    }

    fn apply_local_effect(
        &mut self,
        stack: &mut Vec<F>,
        mut effect: FrameEffect<F, C>,
    ) -> Result<StepResult<C>, E>
    where
        F: Frame<Self, F, C, E>,
        E: LiftFrom<InterpreterError>,
    {
        loop {
            match effect {
                FrameEffect::Continue(frame) => {
                    stack.push(frame);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Push { parent, child } => {
                    stack.push(parent);
                    stack.push(child);
                    return Ok(StepResult::Running);
                }
                FrameEffect::Done => {
                    let parent = match stack.pop() {
                        Some(parent) => parent,
                        None => return Err(E::lift_from(InterpreterError::EmptyFrameStack)),
                    };
                    effect = parent.resume_done(self)?;
                }
                FrameEffect::Complete(completion) => {
                    if let Some(parent) = stack.pop() {
                        effect = parent.resume(completion, self)?;
                    } else {
                        return Ok(StepResult::Complete(completion));
                    }
                }
            }
        }
    }
}
