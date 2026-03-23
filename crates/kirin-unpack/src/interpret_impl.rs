use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{
    Continuation, Interpretable, Interpreter, InterpreterError, write_results,
};
use smallvec::SmallVec;

use crate::{MakeTuple, TupleOp, Unpack};

/// Extension point for tuple packing/unpacking at the interpreter level.
///
/// Dialect authors implement this trait on their value types to define
/// how values are packed into tuples and unpacked from them.
///
/// # Example
///
/// ```ignore
/// impl TupleValue for MyValue {
///     fn make_tuple(values: Vec<Self>) -> Self {
///         MyValue::Tuple(values)
///     }
///     fn unpack(self) -> Result<Vec<Self>, InterpreterError> {
///         match self {
///             MyValue::Tuple(vs) => Ok(vs),
///             _ => Err(InterpreterError::Custom(
///                 "expected tuple value".into(),
///             )),
///         }
///     }
/// }
/// ```
pub trait TupleValue: Sized {
    /// Pack multiple values into a single tuple value.
    fn make_tuple(values: Vec<Self>) -> Self;

    /// Unpack a tuple value into its component values.
    ///
    /// Returns an error if the value is not a tuple.
    fn unpack(self) -> Result<Vec<Self>, InterpreterError>;
}

impl<'ir, I, T> Interpretable<'ir, I> for MakeTuple<T>
where
    I: Interpreter<'ir>,
    I::Value: TupleValue + Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let values: Vec<I::Value> = self
            .args
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let tuple = TupleValue::make_tuple(values);
        interp.write(self.result, tuple)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Unpack<T>
where
    I: Interpreter<'ir>,
    I::Value: TupleValue + Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let source = interp.read(self.source)?;
        let values = TupleValue::unpack(source).map_err(I::Error::from)?;
        write_results(interp, &self.results, &SmallVec::from(values))?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for TupleOp<T>
where
    I: Interpreter<'ir>,
    I::Value: TupleValue + Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            TupleOp::MakeTuple(op) => op.interpret::<L>(interp),
            TupleOp::Unpack(op) => op.interpret::<L>(interp),
        }
    }
}
