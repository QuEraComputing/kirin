//! Interpretation for structured control flow.
//!
//! `scf.if` and `scf.for` are expressed entirely in the framework's scope
//! vocabulary: `if` enters the chosen body block ([`Scope::block`], default
//! finish-on-yield), `for` installs a [`ForHook`] that advances the induction
//! variable and decides repeat/finish in the value domain. Loop fixpoints
//! under abstract interpretation are the engine's job — this dialect carries
//! no analysis-specific code.

use kirin::prelude::{CompileTimeValue, Product, SSAValue};
use kirin_interpreter::dialect::{
    BranchCondition, Ctx, EnvOps, ForwardEffect, ForwardInterp, Interpretable, InterpreterError,
    Scope, ScopeHook, ScopeStep,
};

use crate::{For, ForLoopValue, If, Yield};

impl<I, T> Interpretable<I> for If<T>
where
    I: ForwardInterp,
    I::Value: BranchCondition,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let then_scope = || Scope::block(self.then_body).bind(self.results.iter().copied());
        let else_scope = || Scope::block(self.else_body).bind(self.results.iter().copied());
        match ctx.read(self.condition)?.is_truthy() {
            Some(true) => Ok(ForwardEffect::Enter(then_scope())),
            Some(false) => Ok(ForwardEffect::Enter(else_scope())),
            None => Ok(ForwardEffect::EnterAny(vec![then_scope(), else_scope()])),
        }
    }
}

impl<I, T> Interpretable<I> for For<T>
where
    I: ForwardInterp,
    I::Value: ForLoopValue + 'static,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        let start = ctx.read(self.start)?;
        let end = ctx.read(self.end)?;
        let inits = ctx.read_many(self.init_args.as_slice())?;
        match start.loop_condition(&end) {
            Some(true) => Ok(ForwardEffect::Enter(self.loop_scope(start, inits))),
            Some(false) => {
                // Zero iterations: results are the initial loop-carried values.
                ctx.write_results(self.results.as_slice(), inits)?;
                Ok(ForwardEffect::Next)
            }
            None => Ok(ForwardEffect::EnterAny(vec![
                self.loop_scope(start, inits.clone()),
                Scope::immediate(inits).bind(self.results.iter().copied()),
            ])),
        }
    }
}

impl<T: CompileTimeValue> For<T> {
    fn loop_scope<V, E>(&self, induction: V, carried: Product<V>) -> Scope<V, E>
    where
        V: ForLoopValue + Clone + 'static,
        E: From<InterpreterError>,
    {
        Scope::block(self.body)
            .args(std::iter::once(induction).chain(carried))
            .bind(self.results.iter().copied())
            .on_yield(ForHook {
                end: self.end,
                step: self.step,
            })
    }
}

/// Loop policy for `scf.for`: the induction variable is the first body
/// parameter (read from the joined entry state, so abstract interpretation
/// sees the widened value), advanced by `step` and compared against `end`
/// through [`ForLoopValue`].
struct ForHook {
    end: SSAValue,
    step: SSAValue,
}

impl<V, E> ScopeHook<V, E> for ForHook
where
    V: ForLoopValue + Clone,
    E: From<InterpreterError>,
{
    fn on_yield(
        self: Box<Self>,
        entry: &Product<V>,
        yielded: Product<V>,
        env: &mut dyn EnvOps<V, E>,
    ) -> Result<ScopeStep<V, E>, E> {
        let induction = entry.get(0).cloned().ok_or_else(|| {
            E::from(InterpreterError::Custom(
                "scf.for body is missing its induction parameter",
            ))
        })?;
        let step = env.read(self.step)?;
        let next = induction
            .loop_step(&step)
            .ok_or_else(|| E::from(InterpreterError::LoopStepOverflow))?;
        let end = env.read(self.end)?;
        let args: Product<V> = std::iter::once(next.clone())
            .chain(yielded.iter().cloned())
            .collect();
        match next.loop_condition(&end) {
            Some(true) => Ok(ScopeStep::Repeat { args, hook: self }),
            Some(false) => Ok(ScopeStep::Finish(yielded)),
            None => Ok(ScopeStep::RepeatOrFinish {
                args,
                results: yielded,
                hook: self,
            }),
        }
    }
}

impl<I, T> Interpretable<I> for Yield<T>
where
    I: ForwardInterp,
    T: CompileTimeValue,
{
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<I::Effect, I::Error> {
        Ok(ForwardEffect::Yield(ctx.read_many(self.values.as_slice())?))
    }
}
