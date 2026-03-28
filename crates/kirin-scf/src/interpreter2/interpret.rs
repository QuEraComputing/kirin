use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    Cursor, Interpretable, Interpreter, InterpreterError, ProductValue, ValueStore,
    control::Shell,
    interpreter::{BlockBindings, Position},
};

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

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

impl<'ir, I, T> Interpretable<'ir, I> for For<T>
where
    I: BlockBindings<'ir> + Position<'ir>,
    <I as ValueStore>::Value: ForLoopValue + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        let mut iv = interp.read(self.start)?;
        let end = interp.read(self.end)?;
        let step = interp.read(self.step)?;

        // Initialize loop-carried state: pack init_args into a product (single V).
        let init_values: Vec<<I as ValueStore>::Value> = self
            .init_args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let mut carried = <<I as ValueStore>::Value as ProductValue>::new_product(init_values);

        let stage = interp.stage_info();
        let terminator = self.body.terminator(stage);

        while iv.loop_condition(&end) == Some(true) {
            // Build block args: [iv, ...carried]
            let mut block_args = Vec::with_capacity(1 + self.init_args.len());
            block_args.push(iv.clone());
            if let Some(product) = carried.as_product() {
                block_args.extend(product.iter().cloned());
            } else if !self.init_args.is_empty() {
                block_args.push(carried.clone());
            }

            // Enter inline block
            interp.consume_control(Shell::Push(self.body.into()))?;
            interp.bind_block_args(self.body, &block_args)?;

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

            // Read yield values from terminator
            if let Some(term) = terminator {
                let values: Vec<<I as ValueStore>::Value> = term
                    .arguments(stage)
                    .map(|ssa| interp.read(*ssa))
                    .collect::<Result<_, _>>()?;
                carried = <<I as ValueStore>::Value as ProductValue>::new_product(values);
            }

            // Exit inline block
            interp.consume_control(Shell::Pop)?;

            iv = iv.loop_step(&step).ok_or_else(|| {
                <I as Interpreter<'ir>>::Error::from(InterpreterError::custom(
                    std::io::Error::other("scf.for: induction variable overflow during loop step"),
                ))
            })?;
        }

        // Write final carried state to results
        interp.write_product(&self.results, carried)?;
        Ok(Cursor::Advance)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for StructuredControlFlow<T>
where
    I: BlockBindings<'ir> + Position<'ir>,
    <I as ValueStore>::Value: BranchCondition + ForLoopValue + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor, Self::Error> {
        match self {
            Self::If(op) => op.interpret(interp),
            Self::For(op) => op.interpret(interp),
            Self::Yield(op) => op.interpret(interp),
        }
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
