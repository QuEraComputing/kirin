use kirin_ir::{Block, GetInfo, SSAValue};

use crate::{BlockSeed, InterpreterError, Machine, ProductValue, ValueStore, control::Shell};

use super::{Interpreter, Position, TypedStage};

/// Trait for executing a seed and returning a result value.
///
/// Minimal trait — bounds go on implementations, not on the trait itself.
pub trait Exec<'ir, Seed>: Interpreter<'ir> {
    fn exec(
        &mut self,
        seed: Seed,
    ) -> Result<Option<<Self as ValueStore>::Value>, <Self as Interpreter<'ir>>::Error>;
}

/// Execute a block seed inline: push the block onto the cursor stack, bind
/// the seed-carried arguments, run all non-terminator statements, read the
/// terminator's arguments as a product, and pop the block.
///
/// This is the standard implementation for `Exec<'ir, BlockSeed<V>>`. Concrete
/// interpreter types delegate their `Exec` impl to this function.
pub fn exec_block<'ir, I>(
    interp: &mut I,
    seed: BlockSeed<<I as ValueStore>::Value>,
) -> Result<Option<<I as ValueStore>::Value>, <I as Interpreter<'ir>>::Error>
where
    I: Interpreter<'ir>
        + Position<'ir>
        + TypedStage<'ir>
        + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    Block: Into<<<I as Interpreter<'ir>>::Machine as Machine<'ir>>::Seed>,
{
    let (block, args) = seed.into_parts();
    let stage = interp.stage_info();
    let terminator = block.terminator(stage);

    // Push the block as an inline execution context
    interp.consume_control(Shell::Push(block.into()))?;

    // Bind seed-carried arguments to the block's SSA argument slots
    {
        let stage = interp.stage_info();
        let block_info = block.expect_info(stage);
        let expected = block_info.arguments.len();
        let got = args.len();
        if got != expected {
            return Err(InterpreterError::ArityMismatch { expected, got }.into());
        }
        for (argument, value) in block_info.arguments.iter().zip(args) {
            interp.write(SSAValue::from(*argument), value)?;
        }
    }

    // Run all non-terminator statements
    loop {
        let current = interp.current_statement();
        if current == terminator || current.is_none() {
            break;
        }
        let effect = interp.interpret_current()?;
        let control = interp.consume_effect(effect)?;
        interp.consume_control(control)?;
    }

    // Read terminator yields into a product
    let product = if let Some(term) = terminator {
        let stage = interp.stage_info();
        let values: Vec<<I as ValueStore>::Value> = term
            .arguments(stage)
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        Some(<<I as ValueStore>::Value as ProductValue>::new_product(
            values,
        ))
    } else {
        None
    };

    // Pop the inline context
    interp.consume_control(Shell::Pop)?;
    Ok(product)
}
