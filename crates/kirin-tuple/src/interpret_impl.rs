use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError};
use smallvec::SmallVec;

use crate::{Get, Len, NewTuple, Tuple, Unpack};

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
///             _ => Err(InterpreterError::Custom("expected tuple".into())),
///         }
///     }
///     fn get(&self, index: usize) -> Result<Self, InterpreterError> {
///         match self {
///             MyValue::Tuple(vs) => vs.get(index).cloned()
///                 .ok_or_else(|| InterpreterError::Custom(
///                     format!("tuple index {index} out of bounds").into(),
///                 )),
///             _ => Err(InterpreterError::Custom("expected tuple".into())),
///         }
///     }
///     fn len(&self) -> Result<usize, InterpreterError> {
///         match self {
///             MyValue::Tuple(vs) => Ok(vs.len()),
///             _ => Err(InterpreterError::Custom("expected tuple".into())),
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

    /// Extract a single element by index.
    ///
    /// This does not consume the tuple (takes `&self`), unlike [`unpack`](Self::unpack).
    fn get(&self, index: usize) -> Result<Self, InterpreterError>;

    /// Query the number of elements in the tuple.
    fn len(&self) -> Result<usize, InterpreterError>;

    /// Returns true if the tuple has zero elements.
    fn is_empty(&self) -> Result<bool, InterpreterError> {
        self.len().map(|n| n == 0)
    }

    /// Convert a value to a `usize` index. Used by [`Get`] to interpret the
    /// index operand.
    fn as_index(&self) -> Result<usize, InterpreterError>;

    /// Create a value from a `usize`. Used by [`Len`] to produce the arity
    /// as an SSA value.
    fn from_index(index: usize) -> Self;
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

impl<'ir, I, T> Interpretable<'ir, I> for Get<T>
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
        let index_val = interp.read(self.index)?;
        // The index SSA value must be convertible to a usize.
        // TupleValue::get takes a usize index — the dialect author's
        // value type is responsible for converting from their index
        // representation. We use TupleValue::len as a proxy to validate
        // the source is a tuple, then delegate to get.
        let index = TupleValue::as_index(&index_val).map_err(I::Error::from)?;
        let element = TupleValue::get(&source, index).map_err(I::Error::from)?;
        interp.write(self.result, element)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Len<T>
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
        let arity = TupleValue::len(&source).map_err(I::Error::from)?;
        let result_value = TupleValue::from_index(arity);
        interp.write(self.result, result_value)?;
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
            Tuple::Get(op) => op.interpret::<L>(interp),
            Tuple::Len(op) => op.interpret::<L>(interp),
        }
    }
}
