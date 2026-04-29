use std::marker::PhantomData;

use kirin_ir::{Dialect, TryLiftFrom};

use super::join::join_standard_completion;
use super::{AbstractBranchFrame, AbstractBranchState};
use crate::{
    AbstractValue, BlockFrame, Env, FrameEffect, InterpreterError, ProjectOrSelf,
    StandardCompletion,
};

impl<L, V> AbstractBranchFrame<L, V> {
    pub(super) fn step_abstract<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        L: Dialect,
        F: From<AbstractBranchFrame<L, V>> + From<BlockFrame<L, V>>,
        E: From<InterpreterError>,
        V: AbstractValue,
    {
        let (true_env, true_target, true_arguments) = match &self.state {
            AbstractBranchState::WaitingTrue {
                true_env,
                true_target,
                true_arguments,
                ..
            } => (*true_env, *true_target, true_arguments.clone()),
            AbstractBranchState::WaitingFalse { .. } => {
                return Err(InterpreterError::UnexpectedCompletion {
                    location: self.location,
                    completion: "abstract branch frame stepped after true branch",
                }
                .into());
            }
        };

        let child =
            BlockFrame::<L, V>::new(self.location.stage, true_target, true_env, true_arguments);
        Ok(FrameEffect::Push {
            parent: self.into(),
            child: child.into(),
        })
    }

    pub(super) fn resume_done_abstract<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        E: From<InterpreterError>,
    {
        Err(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "abstract branch child finished without completion",
        }
        .into())
    }

    pub(super) fn resume_abstract<I, F, C, E>(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<F, C>, E>
    where
        I: Env<V, Error = E>,
        L: Dialect,
        F: From<AbstractBranchFrame<L, V>> + From<BlockFrame<L, V>>,
        C: TryLiftFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
        E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
        V: AbstractValue,
    {
        match self.state {
            AbstractBranchState::WaitingTrue {
                true_env,
                false_env,
                false_target,
                false_arguments,
                ..
            } => {
                interp.free(true_env)?;
                let true_completion = completion.project_or_self().map_err(|_| {
                    InterpreterError::UnexpectedCompletion {
                        location: self.location,
                        completion: "abstract branch true path returned dialect completion",
                    }
                })?;
                let child = BlockFrame::<L, V>::new(
                    self.location.stage,
                    false_target,
                    false_env,
                    false_arguments.clone(),
                );
                Ok(FrameEffect::Push {
                    parent: Self {
                        location: self.location,
                        state: AbstractBranchState::WaitingFalse {
                            false_env,
                            true_completion,
                        },
                        marker: PhantomData,
                    }
                    .into(),
                    child: child.into(),
                })
            }
            AbstractBranchState::WaitingFalse {
                false_env,
                true_completion,
                ..
            } => {
                interp.free(false_env)?;
                let false_completion = completion.project_or_self().map_err(|_| {
                    InterpreterError::UnexpectedCompletion {
                        location: self.location,
                        completion: "abstract branch false path returned dialect completion",
                    }
                })?;
                Ok(FrameEffect::Complete(C::try_lift_from(
                    join_standard_completion(true_completion, false_completion)?,
                )?))
            }
        }
    }
}
