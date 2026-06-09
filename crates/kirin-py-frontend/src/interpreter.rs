//! Concrete interpreter wiring for [`PyLang`], so lowered kernels can be *run*
//! (not just printed/parsed). The inner dialects (arith/cmp/constant/scf/
//! function) already provide their `Interpretable` impls; this module only
//! assembles the standard frame/completion/error bundle and a `run` entry, the
//! same shape as `example/toy-lang`'s concrete interpreter.

use kirin::prelude::{Dialect, Pipeline, StageInfo};
use kirin_arith::{ArithConversionError, ArithType};
use kirin_interpreter::{
    AbstractBranchFrame, BlockFrame, CallFrame, Completion, ConcreteBlockTransfer,
    ConcreteInterpreter, Frame, FunctionFrame, HasLocation, InterpreterError, InterpreterProfile,
    LiftError, RegionFrame, SpecializedFunctionFrame, StageFrame, StagedFunctionFrame,
    StandardCompletion, StandardFrame, StatementFrame, expect_single_function_return,
    forward_through,
};
use kirin_scf::interpreter::{ScfCompletion, ScfFrame};

use crate::language::PyLang;

/// Completion payloads produced when a frame finishes: standard
/// (function return) plus structured-control-flow yields.
#[derive(Clone, Debug, PartialEq, Eq, Completion)]
pub enum PyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}

/// Interpreter errors for the Python-lowered language.
#[derive(Debug, LiftError)]
pub enum PyInterpError {
    Core(InterpreterError),
    ArithConversion(ArithConversionError),
}

impl std::fmt::Display for PyInterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(e) => std::fmt::Display::fmt(e, f),
            Self::ArithConversion(e) => std::fmt::Display::fmt(e, f),
        }
    }
}

impl std::error::Error for PyInterpError {}

impl From<kirin_arith::interpreter::DivisionByZero> for PyInterpError {
    fn from(error: kirin_arith::interpreter::DivisionByZero) -> Self {
        Self::Core(error.into())
    }
}

/// Frame stack element: the standard traversal/call frames plus the scf frame.
#[derive(Clone, Debug, PartialEq, Eq, HasLocation, Frame, StageFrame)]
pub enum PyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
    Standard(StandardFrame<L, V, T>),
    Scf(ScfFrame<L, ArithType, V, T>),
}

forward_through! {
    impl[L: Dialect, V, T] for [PyFrame<L, V, T>] via [StandardFrame<L, V, T>]
    from {
        StatementFrame,
        AbstractBranchFrame<L, V>,
        BlockFrame<L, V, T>,
        RegionFrame<L, V, T>,
        CallFrame<L, V>,
        FunctionFrame<L, V>,
        StagedFunctionFrame<L, V>,
        SpecializedFunctionFrame<L, V>,
    }
}

/// Concrete execution of `PyLang` with `i64` values (single `@source` stage).
pub struct PyConcrete;

impl InterpreterProfile for PyConcrete {
    type Stage = StageInfo<PyLang>;
    type Value = i64;
    type Frame = PyFrame<PyLang, i64>;
    type Completion = PyCompletion<i64>;
    type Error = PyInterpError;
}

/// Run a lowered kernel by name with `i64` arguments and return its `i64` result.
pub fn run_i64(
    pipeline: &Pipeline<StageInfo<PyLang>>,
    function_name: &str,
    args: &[i64],
) -> Result<i64, PyInterpError> {
    let mut interp = ConcreteInterpreter::<PyConcrete>::new(pipeline);
    expect_single_function_return(interp.run_function_by_name(
        "source",
        function_name,
        args.iter().copied(),
    )?)
}
