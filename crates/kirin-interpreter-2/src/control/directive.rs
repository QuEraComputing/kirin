use kirin_ir::Block;

use crate::seed::{Args, BlockSeed};

/// Shell-facing control returned after semantic effect consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "controls must be handled by the interpreter"]
pub enum Directive<Stop, Seed = ()> {
    Advance,
    Stay,
    Push(Seed),
    Replace(Seed),
    Pop,
    Stop(Stop),
}

impl<S, Seed> Directive<S, Seed> {
    pub fn map_stop<T>(self, f: impl FnOnce(S) -> T) -> Directive<T, Seed> {
        match self {
            Directive::Advance => Directive::Advance,
            Directive::Stay => Directive::Stay,
            Directive::Push(seed) => Directive::Push(seed),
            Directive::Replace(seed) => Directive::Replace(seed),
            Directive::Pop => Directive::Pop,
            Directive::Stop(stop) => Directive::Stop(f(stop)),
        }
    }

    pub fn map_seed<T>(self, f: impl FnOnce(Seed) -> T) -> Directive<S, T> {
        match self {
            Directive::Advance => Directive::Advance,
            Directive::Stay => Directive::Stay,
            Directive::Push(seed) => Directive::Push(f(seed)),
            Directive::Replace(seed) => Directive::Replace(f(seed)),
            Directive::Pop => Directive::Pop,
            Directive::Stop(stop) => Directive::Stop(stop),
        }
    }

    /// Push an inline block execution context with arguments.
    pub fn push_block<V>(block: Block, args: impl Into<Args<V>>) -> Self
    where
        BlockSeed<V>: Into<Seed>,
    {
        Directive::Push(BlockSeed::new(block, args.into()).into())
    }

    /// Replace the current cursor with a jump to a block with arguments.
    pub fn replace_block<V>(block: Block, args: impl Into<Args<V>>) -> Self
    where
        BlockSeed<V>: Into<Seed>,
    {
        Directive::Replace(BlockSeed::new(block, args.into()).into())
    }

    /// Stop execution with a value.
    pub fn stop(value: S) -> Self {
        Directive::Stop(value)
    }
}

#[cfg(test)]
mod tests {
    use super::Directive;

    #[test]
    fn map_stop_only_transforms_stop_payload() {
        let control: Directive<u16> = Directive::Stop(4_u8).map_stop(u16::from);
        assert_eq!(control, Directive::Stop(4_u16));

        assert_eq!(
            Directive::<u8>::Advance.map_stop(u16::from),
            Directive::<u16>::Advance
        );
        assert_eq!(
            Directive::<u8>::Stay.map_stop(u16::from),
            Directive::<u16>::Stay
        );
        assert_eq!(
            Directive::<u8>::Pop.map_stop(u16::from),
            Directive::<u16>::Pop
        );
    }
}
