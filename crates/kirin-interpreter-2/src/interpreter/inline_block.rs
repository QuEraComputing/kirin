use kirin_ir::Block;

use super::{Interpreter, Position};
use crate::{InterpreterError, ProductValue, ValueStore, control::Shell};

use super::BlockBindings;

/// Inline block execution primitive.
///
/// Executes a block inline: pushes it onto the control stack, binds arguments,
/// runs all non-terminator statements, reads the terminator's arguments as
/// output, and pops the block.
///
/// This primitive is terminator-agnostic — it reads whatever arguments the
/// terminator has. Callers requiring a specific terminator kind (e.g.,
/// `scf.yield`) should validate at compile time or treat `None` as an error.
pub trait InlineBlock<'ir>: BlockBindings<'ir> + Position<'ir>
where
    <Self as ValueStore>::Value: ProductValue,
    <Self as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    /// Execute a block inline and return the product built from the
    /// terminator's arguments.
    ///
    /// Returns `Some(product)` if the block has a terminator, `None` otherwise.
    fn exec_inline_block(
        &mut self,
        block: Block,
        args: impl IntoIterator<Item = <Self as ValueStore>::Value>,
    ) -> Result<Option<<Self as ValueStore>::Value>, <Self as Interpreter<'ir>>::Error>;
}

impl<'ir, T> InlineBlock<'ir> for T
where
    T: BlockBindings<'ir> + Position<'ir>,
    <T as ValueStore>::Value: ProductValue,
    <T as Interpreter<'ir>>::Error: From<InterpreterError>,
{
    fn exec_inline_block(
        &mut self,
        block: Block,
        args: impl IntoIterator<Item = <Self as ValueStore>::Value>,
    ) -> Result<Option<<Self as ValueStore>::Value>, <Self as Interpreter<'ir>>::Error> {
        let stage = self.stage_info();
        let terminator = block.terminator(stage);

        self.consume_control(Shell::Push(block.into()))?;
        self.bind_block_args(block, args)?;

        loop {
            let current = self.current_statement();
            if current == terminator || current.is_none() {
                break;
            }
            let effect = self.interpret_current()?;
            let control = self.consume_effect(effect)?;
            self.consume_control(control)?;
        }

        let product = if let Some(term) = terminator {
            let values: Vec<<Self as ValueStore>::Value> = term
                .arguments(stage)
                .map(|ssa| self.read(*ssa))
                .collect::<Result<_, _>>()?;
            Some(<<Self as ValueStore>::Value as ProductValue>::new_product(
                values,
            ))
        } else {
            None
        };

        self.consume_control(Shell::Pop)?;
        Ok(product)
    }
}
