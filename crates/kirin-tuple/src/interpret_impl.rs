use kirin::prelude::{CompileTimeValue, HasStageInfo};
use kirin_interpreter::{Continuation, Interpretable, Interpreter, InterpreterError, ProductValue};

use crate::{Get, Len, NewTuple, Tuple, Unpack};

/// Convert between interpreter values and `usize` indices.
///
/// Used by [`Get`] (value → index) and [`Len`] (index → value).
/// This is a general integer conversion concern, not specific to products.
pub trait IndexValue: Sized {
    /// Extract a `usize` from this value.
    fn as_index(&self) -> Result<usize, InterpreterError>;

    /// Create a value from a `usize`.
    fn from_index(index: usize) -> Self;
}

impl<'ir, I, T> Interpretable<'ir, I> for NewTuple<T>
where
    I: Interpreter<'ir>,
    I::Value: ProductValue + Clone,
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
        let tuple = <I::Value as ProductValue>::new_product(values);
        interp.write(self.result.into(), tuple)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Unpack<T>
where
    I: Interpreter<'ir>,
    I::Value: ProductValue + Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let source = interp.read(self.source)?;
        let product = source
            .as_product()
            .ok_or_else(|| I::Error::from(InterpreterError::Custom("expected product".into())))?;
        let values: Vec<I::Value> = product.iter().cloned().collect();
        interp.write_many(&self.results, &values)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Get<T>
where
    I: Interpreter<'ir>,
    I::Value: ProductValue + IndexValue + Clone,
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
        let index = IndexValue::as_index(&index_val).map_err(I::Error::from)?;
        let element = ProductValue::get(&source, index).map_err(I::Error::from)?;
        interp.write(self.result.into(), element)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Len<T>
where
    I: Interpreter<'ir>,
    I::Value: ProductValue + IndexValue + Clone,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let source = interp.read(self.source)?;
        let arity = ProductValue::len(&source).map_err(I::Error::from)?;
        let result_value = <I::Value as IndexValue>::from_index(arity);
        interp.write(self.result.into(), result_value)?;
        Ok(Continuation::Continue)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Tuple<T>
where
    I: Interpreter<'ir>,
    I::Value: ProductValue + IndexValue + Clone,
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
