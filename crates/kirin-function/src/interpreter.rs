use kirin::prelude::{CompileTimeValue, HasBottom, HasRegionBody, Product, SSAValue};
use kirin_interpreter::dialect::{
    CallEffect, Callee, DenseBackward, DenseBackwardEffect, DenseBackwardInterp, FunctionBody,
    FunctionEntry, Interp, Interpretable, InterpreterError, SparseBackward, SparseBackwardInterp,
    SparseForward, SparseForwardEffect, SparseForwardInterp,
};

use crate::{
    Bind, CallFunction, CallLike, CallNamed, CallSpecialized, CallStaged, Function, Lambda, Return,
};

/// Backward demand: purity-aware neededness. None of these are marked pure —
/// calls may have effects, so their arguments are unconditional demand roots;
/// `Function`/`Lambda` have no SSA operands, so their rules are inert.
macro_rules! backward_ordinary {
    ($ty:ident) => {
        impl<I, T> Interpretable<I, SparseBackward> for $ty<T>
        where
            I: SparseBackwardInterp,
            I::Value: HasBottom + PartialEq,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                interp.transfer_ordinary(self)
            }
        }
    };
}

backward_ordinary!(Function);
backward_ordinary!(Lambda);
backward_ordinary!(Bind);
backward_ordinary!(CallNamed);
backward_ordinary!(CallFunction);
backward_ordinary!(CallStaged);
backward_ordinary!(CallSpecialized);

/// Classic (weak) per-point liveness: kill defs, gen all uses — the same
/// transfer for calls (purity is irrelevant to per-point live sets).
macro_rules! dense_classic {
    ($ty:ident) => {
        impl<I, T> Interpretable<I, DenseBackward> for $ty<T>
        where
            I: DenseBackwardInterp,
            T: CompileTimeValue,
        {
            fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
                interp.transfer_classic(self)
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
impl<I, T> Interpretable<I, DenseBackward> for Return<T>
where
    I: DenseBackwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        for value in &self.values {
            interp.gen_fact(*value)?;
        }
        Ok(DenseBackwardEffect::Edges(Vec::new()))
    }
}

/// Backward demand: return operands are unconditional roots — they are the
/// function's observable outputs.
impl<I, T> Interpretable<I, SparseBackward> for Return<T>
where
    I: SparseBackwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        for value in &self.values {
            interp.demand(*value)?;
        }
        Ok(interp.effect())
    }
}

impl<I, T> FunctionEntry<I> for Function<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
    }
}

impl<I, T> FunctionEntry<I> for Lambda<T>
where
    I: Interp,
    T: CompileTimeValue,
{
    fn function_entry(
        &self,
        args: Product<I::Value>,
        _interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        Ok(FunctionBody::new(*self.region()).args(args))
    }
}

/// Function definitions are inert at runtime: defining a function does not
/// execute its body. Bodies run when the function is invoked (via
/// [`FunctionEntry`]).
impl<I, T> Interpretable<I, SparseForward> for Function<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
        Ok(SparseForwardEffect::Next)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Lambda<T>
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

impl<I, T> Interpretable<I, SparseForward> for Bind<T>
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

impl<I, T> Interpretable<I, SparseForward> for CallNamed<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Named(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, SparseForward> for CallFunction<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Function(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, SparseForward> for CallStaged<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Staged(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, SparseForward> for CallSpecialized<T>
where
    I: SparseForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        call_effect(self, Callee::Specialized(self.target()), interp)
    }
}

impl<I, T> Interpretable<I, SparseForward> for Return<T>
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
