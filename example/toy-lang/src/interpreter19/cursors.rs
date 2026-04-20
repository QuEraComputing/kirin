use kirin::prelude::HasStageInfo;
use kirin_interpreter_19::algebra::{Lift, Project, SingleStageCursorFor};
use kirin_interpreter_19::block_exec::BlockExecEnv;
use kirin_interpreter_19::control::{Control, CursorExt};
use kirin_interpreter_19::cursor::BlockCursor;
use kirin_interpreter_19::env::AbstractEnv;
use kirin_interpreter_19::error::InterpreterError;
use kirin_interpreter_19::execute::Execute;
use kirin_interpreter_19::interpretable::Interpretable;
use kirin_scf::interpreter19::cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};

use crate::language::{HighLevel, LowLevel};

use super::interp::{AbstractToyVal, ToyVal};

// ---------------------------------------------------------------------------
// HighLevelCursor — concrete coproduct for HighLevel
// ---------------------------------------------------------------------------

pub type SCFCursorHigh<V> = SCFCursor<V, HighLevel>;

pub enum HighLevelCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(SCFCursorHigh<V>),
}

impl<V: Clone> Lift<HighLevelCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Block(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for SCFCursorHigh<V> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for IfCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for ForCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::For(self))
    }
}

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            HighLevelCursor::Block(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> SingleStageCursorFor<HighLevel> for HighLevelCursor<V> {}

impl<E, V> Execute<E> for HighLevelCursor<V>
where
    V: ToyVal,
    E: BlockExecEnv<Value = V, Ext = CursorExt<HighLevelCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<HighLevelCursor<V>>>, E::Error> {
        match self {
            HighLevelCursor::Block(c) => c.execute(env, inbox),
            HighLevelCursor::Scf(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevelAbstractCursor — abstract coproduct for HighLevel.
// KEY CHANGE: uses BlockCursor<V, HighLevel>, not AbstractBlockCursor.
// ---------------------------------------------------------------------------

pub type AbstractSCFCursorHigh<V> = AbstractSCFCursor<V, HighLevel>;

pub enum HighLevelAbstractCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(AbstractSCFCursorHigh<V>),
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Block(self)
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractSCFCursorHigh<V> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(self)
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractIfCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractForCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::For(self))
    }
}

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            HighLevelAbstractCursor::Block(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> SingleStageCursorFor<HighLevel> for HighLevelAbstractCursor<V> {}

impl<E, V> Execute<E> for HighLevelAbstractCursor<V>
where
    V: AbstractToyVal,
    E: AbstractEnv<Value = V, Ext = CursorExt<HighLevelAbstractCursor<V>>>,
    E: BlockExecEnv<Value = V>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<HighLevelAbstractCursor<V>>>, E::Error> {
        match self {
            HighLevelAbstractCursor::Block(c) => c.execute(env, inbox),
            HighLevelAbstractCursor::Scf(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevelAbstract — wrapper enabling SingleStageCursorFor<LowLevel>.
// KEY CHANGE: wraps BlockCursor<V, LowLevel>, not AbstractBlockCursor.
// ---------------------------------------------------------------------------

pub struct LowLevelAbstract<V: Clone>(pub BlockCursor<V, LowLevel>);

impl<V: Clone> SingleStageCursorFor<LowLevel> for LowLevelAbstract<V> {}

impl<E, V> Execute<E> for LowLevelAbstract<V>
where
    V: Clone,
    LowLevel: Interpretable<E>,
    E: AbstractEnv<Value = V, Ext = CursorExt<LowLevelAbstract<V>>>,
    E: BlockExecEnv<Value = V>,
    E::Stages: HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<LowLevelAbstract<V>>>, E::Error> {
        self.0.execute(env, inbox)
    }
}

// ---------------------------------------------------------------------------
// MultiCursor — concrete cursor coproduct spanning both source and lowered
// ---------------------------------------------------------------------------

pub enum MultiCursor<V: Clone> {
    High(BlockCursor<V, HighLevel>),
    Scf(SCFCursorHigh<V>),
    Low(BlockCursor<V, LowLevel>),
}

impl<V: Clone> Lift<MultiCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::High(self)
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for IfCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for ForCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(SCFCursor::For(self))
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for SCFCursorHigh<V> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(self)
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for BlockCursor<V, LowLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Low(self)
    }
}

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for MultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            MultiCursor::High(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<BlockCursor<V, LowLevel>> for MultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, LowLevel>, Self> {
        match self {
            MultiCursor::Low(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<E, V> Execute<E> for MultiCursor<V>
where
    V: ToyVal,
    E: BlockExecEnv<Value = V, Ext = CursorExt<MultiCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel> + HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
    LowLevel: Interpretable<E>,
    BlockCursor<V, HighLevel>: Lift<MultiCursor<V>>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, E::Error> {
        match self {
            MultiCursor::High(c) => c.execute(env, inbox),
            MultiCursor::Scf(c) => c.execute(env, inbox),
            MultiCursor::Low(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractMultiCursor — abstract cursor coproduct spanning source and lowered.
// KEY CHANGE: uses BlockCursor<V, L>, not AbstractBlockCursor<V, L>.
// ---------------------------------------------------------------------------

pub enum AbstractMultiCursor<V: Clone> {
    HighBlock(BlockCursor<V, HighLevel>),
    HighScf(AbstractSCFCursorHigh<V>),
    Low(BlockCursor<V, LowLevel>),
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighBlock(self)
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractSCFCursorHigh<V> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(self)
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractIfCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(AbstractSCFCursor::If(self))
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractForCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(AbstractSCFCursor::For(self))
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for BlockCursor<V, LowLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::Low(self)
    }
}

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            AbstractMultiCursor::HighBlock(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<BlockCursor<V, LowLevel>> for AbstractMultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, LowLevel>, Self> {
        match self {
            AbstractMultiCursor::Low(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<E, V> Execute<E> for AbstractMultiCursor<V>
where
    V: AbstractToyVal,
    E: AbstractEnv<Value = V, Ext = CursorExt<AbstractMultiCursor<V>>>,
    E: BlockExecEnv<Value = V>,
    E::Stages: HasStageInfo<HighLevel> + HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
    LowLevel: Interpretable<E>,
    BlockCursor<V, HighLevel>: Lift<AbstractMultiCursor<V>>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, E::Error> {
        match self {
            AbstractMultiCursor::HighBlock(c) => c.execute(env, inbox),
            AbstractMultiCursor::HighScf(c) => c.execute(env, inbox),
            AbstractMultiCursor::Low(c) => c.execute(env, inbox),
        }
    }
}
