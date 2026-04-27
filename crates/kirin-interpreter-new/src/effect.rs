use kirin_ir::Block;

pub enum FrameEffect<F, C> {
    Continue(F),
    Push { parent: F, child: F },
    Complete(C),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandardCompletion<V> {
    BlockDone,
    RegionDone,
    GraphDone,
    FunctionReturned(V),
}

pub enum StatementEffect<F, C, T> {
    Done,
    Transfer(T),
    Push(F),
    Complete(C),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConcreteTransfer<V> {
    Jump { target: Block, arguments: Vec<V> },
}
