use kirin::prelude::{LiftFrom, Product};
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_interpreter_new::{
    BlockTransfer, EnvIndex, FunctionEntry, Interpretable, InterpreterError, Location,
    StatementEffect,
};
use kirin_scf::StructuredControlFlow;

use crate::language::{HighLevel, LowLevel};

impl<I, F, E, V> FunctionEntry<HighLevel, I, F, E, V> for HighLevel
where
    Lexical<ArithType>: FunctionEntry<HighLevel, I, F, E, V>,
    E: LiftFrom<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lexical(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(E::lift_from(InterpreterError::Custom(
                "expected high-level function body",
            ))),
        }
    }
}

impl<I, F, C, E, X> Interpretable<HighLevel, I, F, C, E, X> for HighLevel
where
    X: BlockTransfer,
    Lexical<ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
    StructuredControlFlow<ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
    Constant<ArithValue, ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
    Arith<ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
    Cmp<ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
    Bitwise<ArithType>: Interpretable<HighLevel, I, F, C, E, X>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Self::Lexical(op) => {
                <Lexical<ArithType> as Interpretable<HighLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Structured(op) => <StructuredControlFlow<ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                X,
            >>::interpret(op, location, env, interp),
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                HighLevel,
                I,
                F,
                C,
                E,
                X,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => {
                <Arith<ArithType> as Interpretable<HighLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Cmp(op) => {
                <Cmp<ArithType> as Interpretable<HighLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Bitwise(op) => {
                <Bitwise<ArithType> as Interpretable<HighLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}

impl<I, F, E, V> FunctionEntry<LowLevel, I, F, E, V> for LowLevel
where
    Lifted<ArithType>: FunctionEntry<LowLevel, I, F, E, V>,
    E: LiftFrom<InterpreterError>,
{
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E> {
        match self {
            Self::Lifted(op) => op.enter_function_body(location, env, interp, args),
            _ => Err(E::lift_from(InterpreterError::Custom(
                "expected low-level function body",
            ))),
        }
    }
}

impl<I, F, C, E, X> Interpretable<LowLevel, I, F, C, E, X> for LowLevel
where
    X: BlockTransfer,
    Lifted<ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
    Constant<ArithValue, ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
    Arith<ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
    Cmp<ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
    Bitwise<ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
    ControlFlow<ArithType>: Interpretable<LowLevel, I, F, C, E, X>,
{
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, X>, E> {
        match self {
            Self::Lifted(op) => {
                <Lifted<ArithType> as Interpretable<LowLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Constant(op) => <Constant<ArithValue, ArithType> as Interpretable<
                LowLevel,
                I,
                F,
                C,
                E,
                X,
            >>::interpret(op, location, env, interp),
            Self::Arith(op) => {
                <Arith<ArithType> as Interpretable<LowLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Cmp(op) => <Cmp<ArithType> as Interpretable<LowLevel, I, F, C, E, X>>::interpret(
                op, location, env, interp,
            ),
            Self::Bitwise(op) => {
                <Bitwise<ArithType> as Interpretable<LowLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
            Self::Cf(op) => {
                <ControlFlow<ArithType> as Interpretable<LowLevel, I, F, C, E, X>>::interpret(
                    op, location, env, interp,
                )
            }
        }
    }
}
