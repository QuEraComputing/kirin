use kirin::prelude::{CompileTimeValue, Dialect, HasStageInfo, StageMeta};
use kirin_interpreter::{BranchCondition, ProductValue};
use kirin_interpreter_8::algebra::Lift;
use kirin_interpreter_8::concrete::ConcreteInterp;
use kirin_interpreter_8::control::{Control, CursorExt};
use kirin_interpreter_8::cursor::BlockCursor;
use kirin_interpreter_8::env::Env;
use kirin_interpreter_8::error::InterpreterError;
use kirin_interpreter_8::semantics::Semantics;

use crate::{For, ForLoopValue, If, StructuredControlFlow, Yield};

use super::cursor::{ForCursor, IfCursor, SCFCursor};

// ---------------------------------------------------------------------------
// If — concrete only
//
// Abstract SCF interpretation is not supported: coherence prevents two
// Semantics impls on the same type bounded by D: ConcreteEnv vs
// D: AbstractEnv. For abstract interpretation use flat CF programs.
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for If<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition + 'static,
    C: 'static,
    BlockCursor<V, L>: Lift<C>,
    IfCursor<V, L>: Lift<C>,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let cond = domain.read_value(self.condition)?;
        let block = match cond.is_truthy() {
            Some(true) => self.then_body,
            Some(false) => self.else_body,
            None => {
                return Err(InterpreterError::UnhandledEffect(
                    "scf.if: nondeterministic condition not supported in concrete mode".into(),
                ));
            }
        };
        let cursor = IfCursor::<V, L>::new(block, self.results.clone(), domain.current_stage());
        Ok(Control::Ext(CursorExt::Push(cursor.lift())))
    }
}

// ---------------------------------------------------------------------------
// For — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for For<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ForLoopValue + ProductValue + 'static,
    C: 'static,
    BlockCursor<V, L>: Lift<C>,
    ForCursor<V, L>: Lift<C>,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let iv = domain.read_value(self.start)?;
        let end = domain.read_value(self.end)?;
        let step = domain.read_value(self.step)?;
        let init_values: Vec<V> = self
            .init_args
            .iter()
            .map(|ssa| domain.read_value(*ssa))
            .collect::<Result<_, _>>()?;
        let init_arg_count = init_values.len();
        let carried = V::new_product(init_values);
        let cursor = ForCursor::<V, L>::builder()
            .iv(iv)
            .end(end)
            .step(step)
            .carried(carried)
            .body(self.body)
            .body_stage(domain.current_stage())
            .init_arg_count(init_arg_count)
            .results(self.results.clone())
            .build();
        Ok(Control::Ext(CursorExt::Push(cursor.lift())))
    }
}

// ---------------------------------------------------------------------------
// Yield — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for Yield<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + ProductValue,
    C: 'static,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        let values: Vec<V> = self
            .values
            .iter()
            .map(|ssa| domain.read_value(*ssa))
            .collect::<Result<_, _>>()?;
        let product = V::new_product(values);
        Ok(Control::Yield(product))
    }
}

// ---------------------------------------------------------------------------
// StructuredControlFlow — concrete only
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C, T> Semantics<ConcreteInterp<'ir, S, L, V, C>> for StructuredControlFlow<T>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone + BranchCondition + ForLoopValue + ProductValue + 'static,
    C: 'static,
    BlockCursor<V, L>: Lift<C>,
    IfCursor<V, L>: Lift<C>,
    ForCursor<V, L>: Lift<C>,
    SCFCursor<V, L>: Lift<C>,
    T: CompileTimeValue,
{
    type Effect = Control<V, CursorExt<C>>;

    fn eval(
        &self,
        domain: &mut ConcreteInterp<'ir, S, L, V, C>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        match self {
            Self::If(op) => op.eval(domain),
            Self::For(op) => op.eval(domain),
            Self::Yield(op) => op.eval(domain),
        }
    }
}
