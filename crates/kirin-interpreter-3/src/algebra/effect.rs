use kirin_ir::{Block, Product, ResultValue, SSAValue};
use smallvec::{SmallVec, smallvec};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect<V, DE> {
    Advance,
    Stay,
    Jump(Block, SmallVec<[V; 2]>),
    BindValue(SSAValue, V),
    BindProduct(Product<ResultValue>, V),
    Return(V),
    Yield(V),
    Stop(V),
    Seq(SmallVec<[Box<Self>; 2]>),
    Machine(DE),
}

impl<V, DE> Effect<V, DE> {
    #[must_use]
    pub fn then(self, next: Self) -> Self {
        Self::Seq(smallvec![Box::new(self), Box::new(next)])
    }
}
