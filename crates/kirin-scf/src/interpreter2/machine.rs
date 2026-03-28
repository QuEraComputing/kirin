use std::convert::Infallible;
use std::marker::PhantomData;

use kirin_interpreter_2::{
    BlockSeed, ConsumeEffect, Cursor, InterpreterError, Lift, Machine, control::Shell,
};

/// Stateless machine for SCF dialect semantics.
///
/// SCF operations (if, for, yield) are total — they never halt the program.
/// `Effect = Cursor<BlockSeed<V>>` (advance/stay/jump), `Stop = Infallible`.
/// `Seed = BlockSeed<V>` so inline block execution carries arguments through
/// the effect pipeline.
pub struct ScfMachine<V> {
    _marker: PhantomData<V>,
}

impl<V> Default for ScfMachine<V> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<'ir, V: 'ir> Machine<'ir> for ScfMachine<V> {
    type Effect = Cursor<BlockSeed<V>>;
    type Stop = Infallible;
    type Seed = BlockSeed<V>;
}

impl<'ir, V: 'ir> ConsumeEffect<'ir> for ScfMachine<V> {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error> {
        Ok(effect.lift())
    }
}
