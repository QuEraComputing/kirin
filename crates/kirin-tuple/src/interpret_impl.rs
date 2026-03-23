use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};
use smallvec::SmallVec;

use crate::{NewTuple, Tuple, Unpack};

/// Extension point for tuple packing/unpacking at the interpreter level.
///
/// Dialect authors implement this trait on their value types to define
/// how values are packed into tuples and unpacked from them.
///
/// # Example
///
/// ```ignore
/// impl TupleValue for MyValue {
///     fn new_tuple(values: Vec<Self>) -> Self {
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
    fn new_tuple(values: Vec<Self>) -> Self;

    /// Unpack a tuple value into its component values.
    ///
    /// Returns an error if the value is not a tuple.
    fn unpack(self) -> Result<Vec<Self>, InterpreterError>;
}

impl<'ir, I, T> Interpretable<'ir, I> for NewTuple<T>
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
        let tuple = TupleValue::new_tuple(values);
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
        interp.write_many(&self.results, &SmallVec::from(values))?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Tuple<T>
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
            Tuple::NewTuple(op) => op.interpret::<L>(interp),
            Tuple::Unpack(op) => op.interpret::<L>(interp),
        }
    }
}
