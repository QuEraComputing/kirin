use kirin_ir::{Block, Product};

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
