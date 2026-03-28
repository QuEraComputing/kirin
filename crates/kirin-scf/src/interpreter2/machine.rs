use std::convert::Infallible;
use std::marker::PhantomData;

use kirin::prelude::Block;
use kirin_interpreter_2::{ConsumeEffect, Cursor, InterpreterError, Lift, Machine, control::Shell};

/// Stateless machine for SCF dialect semantics.
///
/// SCF operations (if, for, yield) are total — they never halt the program.
/// `Effect = Cursor<Block>` (advance/stay/jump), `Stop = Infallible`.
/// `Seed = Block` because inline block execution uses Shell::Push(block).
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
    type Effect = Cursor<Block>;
    type Stop = Infallible;
    type Seed = Block;
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
