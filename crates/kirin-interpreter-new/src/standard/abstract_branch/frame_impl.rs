use std::marker::PhantomData;

use kirin_ir::Dialect;

use super::join::join_standard_completion;
use super::{AbstractBranchFrame, AbstractBranchState};
use crate::{
    AbstractBlockTransfer, AbstractValue, BlockFrame, Env, FrameEffect, InterpreterError,
    ProjectOrSelf, StandardCompletion, StandardFrame,
};

impl<L, V> AbstractBranchFrame<L, V> {
    pub(super) fn step_abstract<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        L: Dialect,
        F: TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
        E: From<InterpreterError>
            + From<<F as TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>,
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
                return Err(E::from(InterpreterError::UnexpectedCompletion {
                    location: self.location,
                    completion: "abstract branch frame stepped after true branch",
                }));
            }
        };

        let child = BlockFrame::<L, V, AbstractBlockTransfer<V>>::new(
            self.location.stage,
            true_target,
            true_env,
            true_arguments,
        );
        Ok(FrameEffect::Push {
            parent: StandardFrame::AbstractBranch(self).try_into()?,
            child: StandardFrame::Block(child).try_into()?,
        })
    }

    pub(super) fn resume_done_abstract<F, C, E>(self) -> Result<FrameEffect<F, C>, E>
    where
        E: From<InterpreterError>,
    {
        Err(E::from(InterpreterError::UnexpectedCompletion {
            location: self.location,
            completion: "abstract branch child finished without completion",
        }))
    }

    pub(super) fn resume_abstract<I, F, C, E>(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<F, C>, E>
    where
        I: Env<V, Error = E>,
        L: Dialect,
        F: TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>,
        C: TryFrom<StandardCompletion<V>> + ProjectOrSelf<StandardCompletion<V>>,
        E: From<InterpreterError>
            + From<<F as TryFrom<StandardFrame<L, V, AbstractBlockTransfer<V>>>>::Error>
            + From<<C as TryFrom<StandardCompletion<V>>>::Error>,
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
                let true_completion = completion.project_or_self().map_err(|_| -> E {
                    E::from(InterpreterError::UnexpectedCompletion {
                        location: self.location,
                        completion: "abstract branch true path returned dialect completion",
                    })
                })?;
                let child = BlockFrame::<L, V, AbstractBlockTransfer<V>>::new(
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
                    .into_standard_frame()
                    .try_into()?,
                    child: StandardFrame::Block(child).try_into()?,
                })
            }
            AbstractBranchState::WaitingFalse {
                false_env,
                true_completion,
                ..
            } => {
                interp.free(false_env)?;
                let false_completion = completion.project_or_self().map_err(|_| -> E {
                    E::from(InterpreterError::UnexpectedCompletion {
                        location: self.location,
                        completion: "abstract branch false path returned dialect completion",
                    })
                })?;
                Ok(FrameEffect::Complete(C::try_from(
                    join_standard_completion(true_completion, false_completion).map_err(E::from)?,
                )?))
            }
        }
    }
}

impl<L, V> AbstractBranchFrame<L, V> {
    fn into_standard_frame(self) -> StandardFrame<L, V, AbstractBlockTransfer<V>> {
        StandardFrame::AbstractBranch(self)
    }
}
