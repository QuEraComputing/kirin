use kirin_ir::Block;
use smallvec::SmallVec;

/// Stack-allocated argument list for block entry.
///
/// Tuned for the common 0–2 argument case (block args, branch args).
pub type Args<V> = SmallVec<[V; 2]>;

/// Public seed for entering a block with arguments.
///
/// This is the composable, value-carrying seed that dialect effects use
/// to request block execution. The interpreter shell converts this into
/// internal cursor state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockSeed<V> {
    block: Block,
    args: Args<V>,
}

impl<V> BlockSeed<V> {
    /// Create a seed to enter a block with the given arguments.
    #[must_use]
    pub fn new(block: Block, args: impl Into<Args<V>>) -> Self {
        Self {
            block,
            args: args.into(),
        }
    }

    /// Create a seed to enter a block with no arguments.
    #[must_use]
    pub fn entry(block: Block) -> Self {
        Self {
            block,
            args: Args::new(),
        }
    }

    /// The target block.
    #[must_use]
    pub fn block(&self) -> Block {
        self.block
    }

    /// The arguments to bind when entering the block.
    #[must_use]
    pub fn args(&self) -> &[V] {
        &self.args
    }

    /// Consume the seed, returning the block and arguments.
    #[must_use]
    pub fn into_parts(self) -> (Block, Args<V>) {
        (self.block, self.args)
    }
}

impl<V> From<Block> for BlockSeed<V> {
    fn from(block: Block) -> Self {
        Self::entry(block)
    }
}
