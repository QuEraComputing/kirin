use crate::{BlockSeed, Machine, ProductValue, control::Directive};

use super::{Interpreter, Position, TypedStage};

/// Trait for executing a seed and returning a result value.
///
/// Minimal trait — bounds go on implementations, not on the trait itself.
pub trait Exec<'ir, Seed>: Interpreter<'ir> {
    fn exec(
        &mut self,
        seed: Seed,
    ) -> Result<Option<Self::Value>, <Self as crate::ValueStore>::Error>;
}

/// Execute a block seed inline: push the block (with args) onto the cursor
/// stack via `Directive::Push`, run all non-terminator statements, read the
/// terminator's arguments as a product, and pop the block.
///
/// Block argument binding happens inside `consume_effect` when the seed is
/// pushed — no separate binding step here.
///
/// This is the standard implementation for `Exec<'ir, BlockSeed<V>>`. Concrete
/// interpreter types delegate their `Exec` impl to this function.
pub fn exec_block<'ir, I>(
    interp: &mut I,
    seed: BlockSeed<I::Value>,
) -> Result<Option<I::Value>, <I as crate::ValueStore>::Error>
where
    I: Interpreter<'ir>
        + Position<'ir>
        + TypedStage<'ir>
        + Machine<'ir, Effect = Directive<<I as Machine<'ir>>::Stop, <I as Machine<'ir>>::Seed>>,
    I::Value: ProductValue,
    BlockSeed<I::Value>: Into<<I as Machine<'ir>>::Seed>,
{
    let block = seed.block();
    let stage = interp.stage_info();
    let terminator = block.terminator(stage);

    // Push the block with its args as an inline execution context.
    // consume_effect will bind the seed-carried arguments to the block's SSA slots.
    interp.consume_effect(Directive::Push(seed.into()))?;

    // Run all non-terminator statements
    loop {
        let current = interp.current_statement();
        if current == terminator || current.is_none() {
            break;
        }
        let effect = interp.interpret_current()?;
        interp.consume_effect(effect)?;
    }

    // Read terminator yields into a product
    let product = if let Some(term) = terminator {
        let stage = interp.stage_info();
        let values: Vec<I::Value> = term
            .arguments(stage)
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        Some(<I::Value as ProductValue>::new_product(values))
    } else {
        None
    };

    // Pop the inline context
    interp.consume_effect(Directive::Pop)?;
    Ok(product)
}
