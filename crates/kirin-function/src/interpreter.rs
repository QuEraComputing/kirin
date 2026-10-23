use kirin::prelude::{CompileTimeValue, HasBottom, Product, SSAValue};
use kirin_interpreter::dialect::{
    CallEffect, Callee, ClassicLiveness, ClassicLivenessInterp, DemandInterp, DenseBackwardEffect,
    ForwardEval, Interpretable, InterpreterError, SparseForwardEffect, SparseForwardInterp,
    StrongDemand,
};

use crate::{
    Bind, CallFunction, CallLike, CallNamed, CallSpecialized, CallStaged, Function, Lambda, Return,
};

/// Backward demand: `Function` has no SSA operands, so its rule is inert.
impl<I, T> Interpretable<I, StrongDemand> for Function<T>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.demand_uses_if_observable(self)
    }
}

/// Closures are unsupported, as in forward evaluation: demand has no
/// convention yet for what a closure value needs from its captures.
impl<I, T> Interpretable<I, StrongDemand> for Lambda<T>
where
    I: DemandInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "first-class lambda values are not yet supported",
        )))
    }
}

impl<I, T> Interpretable<I, StrongDemand> for Bind<T>
where
    I: DemandInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "bind is not yet supported by the interpreter",
        )))
    }
}

/// Backward demand across a call: result `i`'s demand requests the callee's
/// output `i`, and parameter `j`'s demand in the callee becomes demand on
/// argument `j`. A call is activated only through demanded results — an
/// ignored or zero-result call demands nothing (this is a demandedness
/// analysis, not an effect analysis), whatever its purity marking.
fn call_demand<I, T, C>(call: &C, callee: Callee, interp: &mut I) -> Result<I::Effect, I::Error>
where
    I: DemandInterp,
    I::Value: HasBottom + PartialEq,
    T: CompileTimeValue,
    C: CallLike<T>,
{
    let result_values = call
        .results()
        .map(|result| interp.fact(*result))
        .collect::<Result<Product<_>, _>>()?;
    if result_values.iter().all(|fact| *fact == I::Value::bottom()) {
        return Ok(interp.effect());
    }
    let parameter_values = interp.summarize_call(callee, call.stage(), result_values)?;
    let arguments: Vec<SSAValue> = call.arguments().copied().collect();
    if parameter_values.len() != arguments.len() {
        return Err(I::Error::from(InterpreterError::ProductArityMismatch {
            expected: parameter_values.len(),
            actual: arguments.len(),
        }));
    }
    for (argument, fact) in arguments.into_iter().zip(parameter_values) {
        if fact != I::Value::bottom() {
            interp.raise_fact(argument, fact)?;
        }
    }
    Ok(interp.effect())
}

macro_rules! backward_call {
    ($ty:ident, $callee:ident) => {
        impl<I, T> Interpretable<I, StrongDemand> for $ty<T>
        where
            I: DemandInterp,
            I::Value: HasBottom + PartialEq,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                call_demand(self, Callee::$callee(self.target()), interp)
            }
        }
    };
}

backward_call!(CallNamed, Named);
backward_call!(CallFunction, Function);
backward_call!(CallStaged, Staged);
backward_call!(CallSpecialized, Specialized);

/// Classic (weak) per-point liveness: kill defs, gen all uses — the same
/// transfer for calls (purity is irrelevant to per-point live sets).
macro_rules! dense_classic {
    ($ty:ident) => {
        impl<I, T> Interpretable<I, ClassicLiveness> for $ty<T>
        where
            I: ClassicLivenessInterp,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                interp.gen_uses_kill_defs(self)
            }
        }
    };
}

dense_classic!(Function);
dense_classic!(Lambda);
dense_classic!(Bind);
dense_classic!(CallNamed);
dense_classic!(CallFunction);
dense_classic!(CallStaged);
dense_classic!(CallSpecialized);

/// Classic per-point liveness for `ret`: gen the returned values; a function
/// boundary has no CFG edges.
impl<I, T> Interpretable<I, ClassicLiveness> for Return<T>
where
    I: ClassicLivenessInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        for value in &self.values {
            interp.gen_live(*value)?;
        }
        Ok(DenseBackwardEffect::Edges(Vec::new()))
    }
}

/// Backward demand: a return site declares that operand `i` carries the
/// callable's output `i`; the engine merges each requested output into those
/// operands. A return is never an unconditional root.
impl<I, T> Interpretable<I, StrongDemand> for Return<T>
where
    I: DemandInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        interp.declare_return(&self.values)?;
        Ok(interp.effect())
    }
}

/// Function definitions are inert at runtime: defining a function does not
/// execute its body. Invocation discovers the body through
/// `kirin_ir::HasCallableBody`, then the engine initializes and runs it.
impl<I, T> Interpretable<I, ForwardEval> for Function<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(SparseForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Lambda<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "first-class lambda values are not yet supported",
        )))
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Bind<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Err(I::Error::from(InterpreterError::Custom(
            "bind is not yet supported by the interpreter",
        )))
    }
}

fn call_effect<I, T, C>(call: &C, callee: Callee, interp: &mut I) -> Result<I::Effect, I::Error>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
    C: CallLike<T>,
{
    let args = call
        .arguments()
        .map(|argument| interp.read(*argument))
        .collect::<Result<Product<_>, _>>()?;
    let results = call.results().copied().map(SSAValue::from).collect();
    Ok(SparseForwardEffect::Call(CallEffect {
        callee,
        stage: call.stage(),
        args,
        results,
    }))
}

impl<I, T> Interpretable<I, ForwardEval> for CallNamed<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Named(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallFunction<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Function(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallStaged<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Staged(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for CallSpecialized<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Specialized(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, ForwardEval> for Return<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(SparseForwardEffect::Return(
            interp.read_many(self.values.as_slice())?,
        ))
    }
}
