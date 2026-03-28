use std::marker::PhantomData;

use kirin_ir::Block;

use crate::{
    ConsumeEffect, InterpreterError, Lift, Machine,
    control::Shell,
    seed::{Args, BlockSeed},
};

/// Cursor directive for total (non-stopping) dialect operations.
///
/// Contains only cursor directives — no Stop variant. Total dialects
/// return `Cursor` instead of `Shell<Infallible, Seed>`, which avoids Lift
/// trait overlap between identity and Infallible-upcast impls.
///
/// `Cursor` intentionally omits a `Push` variant — total dialects do not
/// spawn nested execution contexts. If needed in the future, extend here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cursor<Seed = ()> {
    /// Advance to the next statement in the block.
    Advance,
    /// Stay at the current cursor position (operation already moved it).
    Stay,
    /// Jump to a different execution point.
    Jump(Seed),
}

impl<Seed: Copy> Copy for Cursor<Seed> {}

impl<Seed> Cursor<Seed> {
    /// Create a jump to a block with arguments.
    pub fn jump<V>(block: Block, args: impl Into<Args<V>>) -> Self
    where
        BlockSeed<V>: Into<Seed>,
    {
        Cursor::Jump(BlockSeed::new(block, args.into()).into())
    }
}

/// Lift a total cursor directive into any `Shell<S, Seed>`.
impl<S, Seed> Lift<Shell<S, Seed>> for Cursor<Seed> {
    fn lift(self) -> Shell<S, Seed> {
        match self {
            Self::Advance => Shell::Advance,
            Self::Stay => Shell::Stay,
            Self::Jump(seed) => Shell::Replace(seed),
        }
    }
}

/// Stateless machine for dialects whose cursor effects lift directly to shell controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stateless<Stop, Seed = ()>(PhantomData<fn() -> (Stop, Seed)>);

impl<Stop, Seed> Default for Stateless<Stop, Seed> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'ir, Stop, Seed> Machine<'ir> for Stateless<Stop, Seed> {
    type Effect = Cursor<Seed>;
    type Stop = Stop;
    type Seed = Seed;
}

impl<'ir, Stop, Seed> ConsumeEffect<'ir> for Stateless<Stop, Seed> {
    type Error = InterpreterError;

    fn consume_effect(
        &mut self,
        effect: Self::Effect,
    ) -> Result<Shell<Self::Stop, Self::Seed>, Self::Error> {
        Ok(effect.lift())
    }
}
