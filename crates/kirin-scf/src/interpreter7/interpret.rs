use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_7::concrete::ConcreteInterp;
use kirin_interpreter_7::control::{Control, ControlExt};
use kirin_interpreter_7::cursor::BlockCursor;
use kirin_interpreter_7::env::Interpretable;
use kirin_interpreter_7::error::InterpreterError;
use kirin_interpreter_7::interp::Interp;
use kirin_interpreter_7::lift::Lift;
use kirin_interpreter_7::store::Store;

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor, SCFCursor};

// ---------------------------------------------------------------------------
// If — concrete only
//
// Abstract SCF interpretation is not supported: coherence prevents two
// Interpretable impls on the same type bounded by E: ConcreteEnv vs
// E: AbstractEnv. For abstract interpretation use flat CF programs.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for If<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition + 'static,
    C: Lift<BlockCursor<V, L>> + Lift<IfCursor<V, L>> + 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        let cond = env.read(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic condition not supported in concrete mode".into(),
                ));
            }
        };
        let cursor = IfCursor::<V, L>::new(block, self.results.clone(), env.current_stage());
        Ok(Control::Ext(ControlExt::Push(C::lift(cursor))))
    }
}

// ---------------------------------------------------------------------------
// For — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for For<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ForLoopValue + ProductValue + 'static,
    C: Lift<BlockCursor<V, L>> + Lift<ForCursor<V, L>> + 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        let iv = env.read(self.start)?;
        let end = env.read(self.end)?;
        let step = env.read(self.step)?;
        let init_values: Vec<V> = self
            .init_args
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let init_arg_count = init_values.len();
        let carried = V::new_product(init_values);
        let cursor = ForCursor::<V, L>::builder()
            .iv(iv)
            .end(end)
            .step(step)
            .carried(carried)
            .body(self.body)
            .body_stage(env.current_stage())
            .init_arg_count(init_arg_count)
            .results(self.results.clone())
            .build();
        Ok(Control::Ext(ControlExt::Push(C::lift(cursor))))
    }
}

// ---------------------------------------------------------------------------
// Yield — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for Yield<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        let values: Vec<V> = self
            .values
            .iter()
            .map(|ssa| env.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = V::new_product(values);
        Ok(Control::Yield(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Interpretable<ConcreteInterp<'ir, S, L, V, C>> for StructuredControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition + ForLoopValue + ProductValue + 'static,
    C: Lift<BlockCursor<V, L>>
        + Lift<IfCursor<V, L>>
        + Lift<ForCursor<V, L>>
        + Lift<SCFCursor<V, L>>
        + 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, ControlExt<C>>;

    fn interpret(
        &self,
        env: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, ControlExt<C>>, InterpreterError> {
        match self {
            Self::If(op) => op.interpret(env),
            Self::For(op) => op.interpret(env),
            Self::Yield(op) => op.interpret(env),
        }
    }
}
