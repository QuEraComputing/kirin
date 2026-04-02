use kirin::prelude::CompileTimeValue;
use kirin_interpreter::BranchCondition;
use kirin_interpreter_4::effect::CursorEffect;
use kirin_interpreter_4::error::InterpreterError;
use kirin_interpreter_4::lift::LiftInto;
use kirin_interpreter_4::traits::{Interpretable, Interpreter, Machine, ValueStore};

use crate::ControlFlow;

impl<I, T> Interpretable<I> for ControlFlow<T>
where
    I: Interpreter + Machine<Error = InterpreterError>,
    <I as ValueStore>::Value: Clone + BranchCondition,
    CursorEffect<<I as ValueStore>::Value>: LiftInto<<I as Machine>::Effect>,
    T: CompileTimeValue,
{
    type Effect = CursorEffect<<I as ValueStore>::Value>;
    type Error = InterpreterError;

    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<CursorEffect<<I as ValueStore>::Value>, InterpreterError> {
        match self {
            ControlFlow::Branch { target, args } => {
                let values = interp.read_many(args)?;
                Ok(CursorEffect::Jump(target.target(), values))
            }
            ControlFlow::ConditionalBranch {
                condition,
                true_target,
                true_args,
                false_target,
                false_args,
            } => {
                let cond = interp.read(*condition)?;
                let (block, args) =
                    match cond.is_truthy() {
                        Some(true) => (true_target.target(), true_args.as_slice()),
                        Some(false) => (false_target.target(), false_args.as_slice()),
                        None => return Err(InterpreterError::UnhandledEffect(
                            "nondeterministic branch conditions are not supported in interpreter4"
                                .into(),
                        )),
                    };
                let values = interp.read_many(args)?;
                Ok(CursorEffect::Jump(block, values))
            }
            Self::__Phantom(..) => unreachable!(),
        }
    }
}
