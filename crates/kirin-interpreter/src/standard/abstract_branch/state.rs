use std::marker::PhantomData;

use kirin_ir::{Block, Product};

use crate::{EnvIndex, HasLocation, Location, Position, StandardCompletion, Traversal};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbstractBranchFrame<L, V> {
    pub location: Location,
    pub state: AbstractBranchState<V>,
    pub(super) marker: PhantomData<fn() -> L>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbstractBranchState<V> {
    WaitingTrue {
        true_env: EnvIndex,
        true_target: Block,
        true_arguments: Product<V>,
        false_env: EnvIndex,
        false_target: Block,
        false_arguments: Product<V>,
    },
    WaitingFalse {
        false_env: EnvIndex,
        true_completion: StandardCompletion<V>,
    },
}

impl<L, V> AbstractBranchFrame<L, V> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        true_env: EnvIndex,
        true_target: Block,
        true_arguments: Product<V>,
        false_env: EnvIndex,
        false_target: Block,
        false_arguments: Product<V>,
    ) -> Self {
        Self {
            location: Location::new(
                stage,
                Position::Block {
                    block: true_target,
                    traversal: Traversal::Entry,
                },
            ),
            state: AbstractBranchState::WaitingTrue {
                true_env,
                true_target,
                true_arguments,
                false_env,
                false_target,
                false_arguments,
            },
            marker: PhantomData,
        }
    }
}

impl<L, V> HasLocation for AbstractBranchFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}
