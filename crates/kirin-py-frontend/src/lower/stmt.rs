use kirin::prelude::*;
use kirin_arith::ArithType;
use kirin_function::Return;

use super::BlockBuf;
use super::control::{lower_for, lower_if};
use super::expr::lower_expr;
use crate::ast::Stmt;
use crate::error::LowerError;
use crate::language::PyLang;
use crate::scope::Frame;

/// Lower a sequence of statements into `buf`. Stops at the first `return`
/// (which becomes the block terminator); `if`/`for` are regular statements so
/// lowering continues in the same block afterward (scf is structured).
pub(crate) fn lower_stmts(
    ctx: &mut BuilderStageInfo<PyLang>,
    stmts: &[Stmt],
    frame: &mut Frame,
    buf: &mut BlockBuf,
) -> Result<(), LowerError> {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, value } => {
                let v = lower_expr(ctx, value, frame, buf)?;
                frame.define(target, v);
            }
            Stmt::Return { value } => {
                let values: Vec<SSAValue> = match value {
                    Some(e) => vec![lower_expr(ctx, e, frame, buf)?],
                    None => vec![],
                };
                let ret = Return::<ArithType>::new(ctx, values);
                buf.set_terminator(ret.id);
                break;
            }
            Stmt::Expr(e) => {
                lower_expr(ctx, e, frame, buf)?;
            }
            Stmt::If { test, body, orelse } => lower_if(ctx, test, body, orelse, frame, buf)?,
            Stmt::For { target, iter, body } => lower_for(ctx, target, iter, body, frame, buf)?,
        }
    }
    Ok(())
}
