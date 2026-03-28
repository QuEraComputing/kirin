use kirin::prelude::{Block, CompileTimeValue};
use kirin_interpreter::BranchCondition;
use kirin_interpreter_2::{
    Args, BlockSeed, Cursor, Exec, Interpretable, Interpreter, InterpreterError, ProductValue,
    ValueStore,
};
use smallvec::SmallVec;

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir, I, T> Interpretable<'ir, I> for If<T>
where
    I: Exec<'ir, BlockSeed<<I as ValueStore>::Value>>
        + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: BranchCondition + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor<Block>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor<Block>, Self::Error> {
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

        if let Some(product) = interp.exec(BlockSeed::entry(block))? {
            interp.write_product(&self.results, product)?;
        }
        Ok(Cursor::Advance)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for For<T>
where
    I: Exec<'ir, BlockSeed<<I as ValueStore>::Value>>
        + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: ForLoopValue + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor<Block>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor<Block>, Self::Error> {
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

        let mut block_args: SmallVec<[_; 8]> = SmallVec::with_capacity(1 + self.init_args.len());
        while iv.loop_condition(&end) == Some(true) {
            block_args.push(iv.clone());
            if let Some(product) = carried.as_product() {
                block_args.extend(product.iter().cloned());
            } else if !self.init_args.is_empty() {
                block_args.push(carried.clone());
            }

            let args: Args<_> = block_args.drain(..).collect();
            if let Some(product) = interp.exec(BlockSeed::new(self.body, args))? {
                carried = product;
            }

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
    I: Exec<'ir, BlockSeed<<I as ValueStore>::Value>>
        + ValueStore<Error = <I as Interpreter<'ir>>::Error>,
    <I as ValueStore>::Value: BranchCondition + ForLoopValue + ProductValue,
    <I as Interpreter<'ir>>::Error: From<InterpreterError>,
    T: CompileTimeValue,
{
    type Effect = Cursor<Block>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, interp: &mut I) -> Result<Cursor<Block>, Self::Error> {
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
    type Effect = Cursor<Block>;
    type Error = <I as Interpreter<'ir>>::Error;

    fn interpret(&self, _interp: &mut I) -> Result<Cursor<Block>, Self::Error> {
        Err(unsupported(
            "scf.yield has no independent semantics; \
             it may only appear as a terminator inside scf.if or scf.for body blocks",
        )
        .into())
    }
}
