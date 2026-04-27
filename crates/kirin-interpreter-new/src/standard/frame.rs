use crate::{Frame, FrameEffect, HasLocation, Location};

use super::{BlockFrame, RegionFrame, StatementFrame};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandardFrame<L, V> {
    Statement(StatementFrame),
    Block(BlockFrame<L, V>),
    Region(RegionFrame<L, V>),
}

impl<L, V> From<StatementFrame> for StandardFrame<L, V> {
    fn from(frame: StatementFrame) -> Self {
        Self::Statement(frame)
    }
}

impl<L, V> From<BlockFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: BlockFrame<L, V>) -> Self {
        Self::Block(frame)
    }
}

impl<L, V> From<RegionFrame<L, V>> for StandardFrame<L, V> {
    fn from(frame: RegionFrame<L, V>) -> Self {
        Self::Region(frame)
    }
}

impl<L, V> HasLocation for StandardFrame<L, V> {
    fn location(&self) -> Location {
        match self {
            Self::Statement(frame) => frame.location(),
            Self::Block(frame) => frame.location(),
            Self::Region(frame) => frame.location(),
        }
    }
}

impl<I, L, C, E, V> Frame<I, StandardFrame<L, V>, C, E> for StandardFrame<L, V>
where
    StatementFrame: Frame<I, StandardFrame<L, V>, C, E>,
    BlockFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
    RegionFrame<L, V>: Frame<I, StandardFrame<L, V>, C, E>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<StandardFrame<L, V>, C>, E> {
        match self {
            Self::Statement(frame) => frame.step(interp),
            Self::Block(frame) => frame.step(interp),
            Self::Region(frame) => frame.step(interp),
        }
    }

    fn resume(
        self,
        completion: C,
        interp: &mut I,
    ) -> Result<FrameEffect<StandardFrame<L, V>, C>, E> {
        match self {
            Self::Statement(frame) => frame.resume(completion, interp),
            Self::Block(frame) => frame.resume(completion, interp),
            Self::Region(frame) => frame.resume(completion, interp),
        }
    }
}
