use kirin_ir::{Block, LiftFrom, Product};

use crate::{InterpreterError, ProjectOrSelf};

pub enum FrameEffect<F, C> {
    Continue(F),
    Push { parent: F, child: F },
    Done,
    Complete(C),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandardCompletion<V> {
    BlockDone,
    RegionDone,
    GraphDone,
    FunctionReturned(Product<V>),
}

pub fn expect_single_function_return<V, C, E>(completion: C) -> Result<V, E>
where
    C: ProjectOrSelf<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>,
{
    match completion.project_or_self() {
        Ok(StandardCompletion::FunctionReturned(product)) => {
            if product.len() == 1 {
                Ok(product.into_iter().next().unwrap())
            } else {
                Err(E::lift_from(InterpreterError::ProductArityMismatch {
                    expected: 1,
                    actual: product.len(),
                }))
            }
        }
        Ok(_) => Err(E::lift_from(InterpreterError::ExpectedFunctionReturn(
            "non-function completion",
        ))),
        Err(_) => Err(E::lift_from(InterpreterError::ExpectedFunctionReturn(
            "completion did not project to StandardCompletion",
        ))),
    }
}

pub enum StatementEffect<F, C, T> {
    Done,
    Transfer(T),
    Push(F),
    Complete(C),
}

pub trait BlockTransfer {
    type Value;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcreteBlockTransfer<V> {
    Jump {
        target: Block,
        arguments: Product<V>,
    },
}

impl<V> BlockTransfer for ConcreteBlockTransfer<V> {
    type Value = V;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbstractBlockTransfer<V> {
    Jump {
        target: Block,
        arguments: Product<V>,
    },
    Branch {
        true_target: Block,
        true_arguments: Product<V>,
        false_target: Block,
        false_arguments: Product<V>,
    },
}

impl<V> BlockTransfer for AbstractBlockTransfer<V> {
    type Value = V;
}
