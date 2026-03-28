use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    Cursor, Interpretable, Interpreter, InterpreterError, ProductValue, ValueStore,
    control::Shell,
    interpreter::{BlockBindings, Position},
};

use crate::{If, Yield};

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for If<T>
where
    I: BlockBindings<'ir> + Position<'ir>,
    <I as ValueStore>::Value: BranchCondition + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        let cond = interp.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(unsupported(
                    "scf.if: nondeterministic conditions not supported in interpreter-2",
                )
                .into());
            }
        };

        let stage = interp.stage_info();
        let terminator = block.terminator(stage);

        // Enter inline block
        interp.consume_control(Shell::Push(block.into()))?;
        interp.bind_block_args(block, &[])?;

        // Run non-terminator statements
        loop {
            let current = interp.current_statement();
            if current == terminator || current.is_none() {
                break;
            }
            let effect = interp.interpret_current()?;
            let control = interp.consume_effect(effect)?;
            interp.consume_control(control)?;
        }

        // Read yield values from terminator's IR definition
        if let Some(term) = terminator {
            let values: Vec<<I as ValueStore>::Value> = term
                .arguments(stage)
                .map(|ssa| interp.read(*ssa))
                .collect::<Result<_, _>>()?;
            let product = <<I as ValueStore>::Value as ProductValue>::new_product(values);
            interp.write_product(&self.results, product)?;
        }

        // Exit inline block
        interp.consume_control(Shell::Pop)?;
        Ok(Cursor::Advance)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Yield<T>
where
    I: Interpreter<'ir>,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Cursor, Self::Error> {
        Err(unsupported(
            "scf.yield has no independent semantics; \
             it may only appear as a terminator inside scf.if or scf.for body blocks",
        )
        .into())
    }
}
