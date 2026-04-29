use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_interpreter_new::{
    BlockTransfer, EnvIndex, FunctionBodyEntry, Interpretable, InterpreterError, Location,
    StatementEffect,
};
use kirin_scf::StructuredControlFlow;

use crate::language::{HighLevel, LowLevel};

impl<I, F, E, V> FunctionBodyEntry<HighLevel, I, F, E, V> for HighLevel
where
    Lexical<ArithType>: FunctionBodyEntry<HighLevel, I, F, E, V>,
    E: From<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lexical(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(InterpreterError::Custom("expected high-level function body").into()),
        }
    }
}

impl<I, F, C, E, V> Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>> for HighLevel
where
    Lexical<ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
    StructuredControlFlow<ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
    Constant<ArithValue, ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
    Arith<ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
    Cmp<ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
    Bitwise<ArithType>: Interpretable<HighLevel, I, F, C, E, BlockTransfer<V>>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            Self::Lexical(op) => <Lexical<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Structured(op) => <StructuredControlFlow<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => <Arith<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cmp(op) => <Cmp<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Bitwise(op) => <Bitwise<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
        }
    }
}

impl<I, F, E, V> FunctionBodyEntry<LowLevel, I, F, E, V> for LowLevel
where
    Lifted<ArithType>: FunctionBodyEntry<LowLevel, I, F, E, V>,
    E: From<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Vec<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lifted(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(InterpreterError::Custom("expected low-level function body").into()),
        }
    }
}

impl<I, F, C, E, V> Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>> for LowLevel
where
    Lifted<ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
    Constant<ArithValue, ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
    Arith<ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
    Cmp<ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
    Bitwise<ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
    ControlFlow<ArithType>: Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, BlockTransfer<V>>, E> {
        match self {
            Self::Lifted(op) => <Lifted<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => <Arith<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cmp(op) => {
                <Cmp<ArithType> as Interpretable<LowLevel, I, F, C, E, BlockTransfer<V>>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Bitwise(op) => <Bitwise<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
            Self::Cf(op) => <ControlFlow<ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                BlockTransfer<V>,
            >>::interpret(op, location, env, interp),
        }
    }
}
