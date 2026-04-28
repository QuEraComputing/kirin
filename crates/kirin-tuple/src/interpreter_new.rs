use kirin::prelude::{CompileTimeValue, Dialect, SSAValue};
use kirin_interpreter_new::{
    ConcreteTransfer, Env, Interpretable, InterpreterError, Location, ProductValue, StatementEffect,
};

use crate::{Get, Len, NewTuple, Tuple, Unpack};

pub trait TupleIndexValue: Sized {
    fn as_tuple_index(&self) -> Option<usize>;
    fn from_tuple_index(index: usize) -> Self;
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for NewTuple<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: ProductValue,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let values = self
            .args
            .iter()
            .map(|arg| interp.read(env, *arg))
            .collect::<Result<Vec<_>, _>>()?;
        interp.write(env, SSAValue::from(self.result), V::new_product(values))?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Unpack<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: ProductValue,
    E: From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let source = interp.read(env, self.source)?;
        let results = self
            .results
            .iter()
            .copied()
            .map(SSAValue::from)
            .collect::<Vec<_>>();
        interp.write_product(env, results.as_slice(), source)?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Get<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: ProductValue + TupleIndexValue,
    E: From<ExpectedTuple> + From<InvalidTupleIndex> + From<TupleIndexOutOfBounds>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let source = interp.read(env, self.source)?;
        let index = interp
            .read(env, self.index)?
            .as_tuple_index()
            .ok_or(InvalidTupleIndex)?;
        let value = source
            .as_product()
            .ok_or(ExpectedTuple)?
            .get(index)
            .cloned()
            .ok_or(TupleIndexOutOfBounds)?;
        interp.write(env, SSAValue::from(self.result), value)?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Len<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: ProductValue + TupleIndexValue,
    E: From<ExpectedTuple>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        _location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        let source = interp.read(env, self.source)?;
        let len = source.as_product().ok_or(ExpectedTuple)?.len();
        interp.write(env, SSAValue::from(self.result), V::from_tuple_index(len))?;
        Ok(StatementEffect::Done)
    }
}

impl<L, I, F, C, E, V, T> Interpretable<L, I, F, C, E, ConcreteTransfer<V>> for Tuple<T>
where
    L: Dialect,
    I: Env<V, Error = E>,
    V: ProductValue + TupleIndexValue,
    E: From<ExpectedTuple>
        + From<InvalidTupleIndex>
        + From<TupleIndexOutOfBounds>
        + From<InterpreterError>,
    T: CompileTimeValue,
{
    fn interpret(
        &self,
        location: Location,
        env: kirin_interpreter_new::EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, ConcreteTransfer<V>>, E> {
        match self {
            Tuple::NewTuple(op) => {
                <NewTuple<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Tuple::Unpack(op) => {
                <Unpack<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Tuple::Get(op) => {
                <Get<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Tuple::Len(op) => {
                <Len<T> as Interpretable<L, I, F, C, E, ConcreteTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedTuple;

impl std::fmt::Display for ExpectedTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expected tuple value")
    }
}

impl std::error::Error for ExpectedTuple {}

impl From<ExpectedTuple> for InterpreterError {
    fn from(_: ExpectedTuple) -> Self {
        Self::Custom("expected tuple value")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidTupleIndex;

impl std::fmt::Display for InvalidTupleIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid tuple index")
    }
}

impl std::error::Error for InvalidTupleIndex {}

impl From<InvalidTupleIndex> for InterpreterError {
    fn from(_: InvalidTupleIndex) -> Self {
        Self::Custom("invalid tuple index")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TupleIndexOutOfBounds;

impl std::fmt::Display for TupleIndexOutOfBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tuple index out of bounds")
    }
}

impl std::error::Error for TupleIndexOutOfBounds {}

impl From<TupleIndexOutOfBounds> for InterpreterError {
    fn from(_: TupleIndexOutOfBounds) -> Self {
        Self::Custom("tuple index out of bounds")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TupleArityMismatch;

impl std::fmt::Display for TupleArityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tuple arity mismatch")
    }
}

impl std::error::Error for TupleArityMismatch {}

impl From<TupleArityMismatch> for InterpreterError {
    fn from(_: TupleArityMismatch) -> Self {
        Self::Custom("tuple arity mismatch")
    }
}
