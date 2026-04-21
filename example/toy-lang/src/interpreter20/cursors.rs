use kirin::prelude::HasStageInfo;
use kirin_interpreter_20::algebra::{Lift, SingleStageCursorFor, TryLiftFrom, TryProjectTo};
use kirin_interpreter_20::block_exec::BlockExecEnv;
use kirin_interpreter_20::control::{Control, CursorExt};
use kirin_interpreter_20::cursor::BlockCursor;
use kirin_interpreter_20::env::AbstractEnv;
use kirin_interpreter_20::error::InterpreterError;
use kirin_interpreter_20::execute::Execute;
use kirin_interpreter_20::interpretable::Interpretable;
use kirin_scf::interpreter20::cursor::{
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

impl<V: Clone> TryLiftFrom<BlockCursor<V, HighLevel>> for HighLevelCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelCursor::Block(c))
    }
}

impl<V: Clone> TryLiftFrom<SCFCursorHigh<V>> for HighLevelCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: SCFCursorHigh<V>) -> Result<Self, Self::Error> {
        Ok(HighLevelCursor::Scf(c))
    }
}

impl<V: Clone> TryLiftFrom<IfCursor<V, HighLevel>> for HighLevelCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: IfCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelCursor::Scf(SCFCursor::If(c)))
    }
}

impl<V: Clone> TryLiftFrom<ForCursor<V, HighLevel>> for HighLevelCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: ForCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelCursor::Scf(SCFCursor::For(c)))
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, HighLevel>> for HighLevelCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, HighLevel>, Self> {
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
// ---------------------------------------------------------------------------

pub type AbstractSCFCursorHigh<V> = AbstractSCFCursor<V, HighLevel>;

pub enum HighLevelAbstractCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(AbstractSCFCursorHigh<V>),
}

impl<V: Clone> TryLiftFrom<BlockCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelAbstractCursor::Block(c))
    }
}

impl<V: Clone> TryLiftFrom<AbstractSCFCursorHigh<V>> for HighLevelAbstractCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractSCFCursorHigh<V>) -> Result<Self, Self::Error> {
        Ok(HighLevelAbstractCursor::Scf(c))
    }
}

impl<V: Clone> TryLiftFrom<AbstractIfCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractIfCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelAbstractCursor::Scf(AbstractSCFCursor::If(c)))
    }
}

impl<V: Clone> TryLiftFrom<AbstractForCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractForCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(HighLevelAbstractCursor::Scf(AbstractSCFCursor::For(c)))
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, HighLevel>, Self> {
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

impl<V: Clone> TryLiftFrom<BlockCursor<V, HighLevel>> for MultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(MultiCursor::High(c))
    }
}

impl<V: Clone> TryLiftFrom<IfCursor<V, HighLevel>> for MultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: IfCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(MultiCursor::Scf(SCFCursor::If(c)))
    }
}

impl<V: Clone> TryLiftFrom<ForCursor<V, HighLevel>> for MultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: ForCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(MultiCursor::Scf(SCFCursor::For(c)))
    }
}

impl<V: Clone> TryLiftFrom<SCFCursorHigh<V>> for MultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: SCFCursorHigh<V>) -> Result<Self, Self::Error> {
        Ok(MultiCursor::Scf(c))
    }
}

impl<V: Clone> TryLiftFrom<BlockCursor<V, LowLevel>> for MultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, LowLevel>) -> Result<Self, Self::Error> {
        Ok(MultiCursor::Low(c))
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, HighLevel>> for MultiCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            MultiCursor::High(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, LowLevel>> for MultiCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, LowLevel>, Self> {
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
// ---------------------------------------------------------------------------

pub enum AbstractMultiCursor<V: Clone> {
    HighBlock(BlockCursor<V, HighLevel>),
    HighScf(AbstractSCFCursorHigh<V>),
    Low(BlockCursor<V, LowLevel>),
}

impl<V: Clone> TryLiftFrom<BlockCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(AbstractMultiCursor::HighBlock(c))
    }
}

impl<V: Clone> TryLiftFrom<AbstractSCFCursorHigh<V>> for AbstractMultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractSCFCursorHigh<V>) -> Result<Self, Self::Error> {
        Ok(AbstractMultiCursor::HighScf(c))
    }
}

impl<V: Clone> TryLiftFrom<AbstractIfCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractIfCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(AbstractMultiCursor::HighScf(AbstractSCFCursor::If(c)))
    }
}

impl<V: Clone> TryLiftFrom<AbstractForCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: AbstractForCursor<V, HighLevel>) -> Result<Self, Self::Error> {
        Ok(AbstractMultiCursor::HighScf(AbstractSCFCursor::For(c)))
    }
}

impl<V: Clone> TryLiftFrom<BlockCursor<V, LowLevel>> for AbstractMultiCursor<V> {
    type Error = kirin_interpreter_20::algebra::LiftError;
    fn try_lift_from(c: BlockCursor<V, LowLevel>) -> Result<Self, Self::Error> {
        Ok(AbstractMultiCursor::Low(c))
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            AbstractMultiCursor::HighBlock(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> TryProjectTo<BlockCursor<V, LowLevel>> for AbstractMultiCursor<V> {
    type Error = Self;
    fn try_project_to(self) -> Result<BlockCursor<V, LowLevel>, Self> {
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
