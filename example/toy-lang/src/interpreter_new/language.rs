use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_interpreter_new::{BlockTransfer, EnvIndex, Interpretable, Location, StatementEffect};
use kirin_scf::StructuredControlFlow;

use crate::language::{HighLevel, LowLevel};

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
