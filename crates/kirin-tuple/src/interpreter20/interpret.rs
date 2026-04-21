use kirin::prelude::CompileTimeValue;
use kirin_interpreter::ProductValue;
use kirin_interpreter_20::control::Control;
use kirin_interpreter_20::env::Env;
use kirin_interpreter_20::error::InterpreterError;
use kirin_interpreter_20::interpretable::Interpretable;

use crate::{Get, Len, NewTuple, Tuple, Unpack};

pub trait IndexValue: Sized {
    fn as_index(&self) -> Result<usize, InterpreterError>;
    fn from_index(index: usize) -> Self;
}

impl<E, T> Interpretable<E> for NewTuple<T>
where
    E: Env,
    E::Value: ProductValue + Clone,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let values: Vec<E::Value> = self
            .args
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let tuple = <E::Value as ProductValue>::new_product(values);
        env.write_result(self.result, tuple)?;
        Ok(Control::Advance)
    }
}

impl<E, T> Interpretable<E> for Unpack<T>
where
    E: Env,
    E::Value: ProductValue + Clone,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let source = env.read(self.source)?;
        let product = source
            .as_product()
            .ok_or_else(|| E::Error::from(InterpreterError::Custom("expected product".into())))?;
        let values: Vec<E::Value> = product.iter().cloned().collect();
        for (result, v) in self.results.iter().zip(values) {
            env.write_result(*result, v)?;
        }
        Ok(Control::Advance)
    }
}

impl<E, T> Interpretable<E> for Get<T>
where
    E: Env,
    E::Value: ProductValue + IndexValue + Clone,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let source = env.read(self.source)?;
        let index_val = env.read(self.index)?;
        let index = IndexValue::as_index(&index_val).map_err(E::Error::from)?;
        let element = source
            .as_product()
            .and_then(|p| p.get(index).cloned())
            .ok_or_else(|| {
                E::Error::from(InterpreterError::Custom(
                    format!("product index {index} out of bounds").into(),
                ))
            })?;
        env.write_result(self.result, element)?;
        Ok(Control::Advance)
    }
}

impl<E, T> Interpretable<E> for Len<T>
where
    E: Env,
    E::Value: ProductValue + IndexValue + Clone,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        let source = env.read(self.source)?;
        let arity = source
            .as_product()
            .map(|p| p.len())
            .ok_or_else(|| E::Error::from(InterpreterError::Custom("expected product".into())))?;
        let result_value = <E::Value as IndexValue>::from_index(arity);
        env.write_result(self.result, result_value)?;
        Ok(Control::Advance)
    }
}

impl<E, T> Interpretable<E> for Tuple<T>
where
    E: Env,
    E::Value: ProductValue + IndexValue + Clone,
    T: CompileTimeValue,
{
    fn eval(&self, env: &mut E) -> Result<Control<E::Value, E::Ext>, E::Error> {
        match self {
            Tuple::NewTuple(op) => op.eval(env),
            Tuple::Unpack(op) => op.eval(env),
            Tuple::Get(op) => op.eval(env),
            Tuple::Len(op) => op.eval(env),
        }
    }
}
