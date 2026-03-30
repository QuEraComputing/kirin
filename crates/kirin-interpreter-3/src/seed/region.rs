use kirin_ir::{Dialect, Region};
use smallvec::{SmallVec, smallvec};

use crate::{Effect, Execute, Machine, ProductValue};

use super::super::runtime::SingleStage;
use super::BlockSeed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSeed<V> {
    region: Region,
    args: SmallVec<[V; 2]>,
}

impl<V> RegionSeed<V> {
    #[must_use]
    pub fn new(region: Region, args: impl Into<SmallVec<[V; 2]>>) -> Self {
        Self {
            region,
            args: args.into(),
        }
    }

    #[must_use]
    pub fn entry(region: Region) -> Self {
        Self {
            region,
            args: smallvec![],
        }
    }
}

impl<'ir, L, V, M, S> Execute<SingleStage<'ir, L, V, M, S>> for RegionSeed<V>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    type Output = Effect<V, M::Effect>;

    fn execute(
        self,
        interp: &mut SingleStage<'ir, L, V, M, S>,
    ) -> Result<Self::Output, <SingleStage<'ir, L, V, M, S> as Machine>::Error> {
        let mut block = interp.region_entry_block(self.region)?;
        let mut args = self.args;

        loop {
            match BlockSeed::new(block, args).execute(interp)? {
                Effect::Jump(next, next_args) => {
                    block = next;
                    args = next_args;
                }
                terminal => return Ok(terminal),
            }
        }
    }
}
