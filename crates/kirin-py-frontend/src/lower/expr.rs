use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::Call;

use super::BlockBuf;
use crate::ast::{BinOp, CmpOp, Const, Expr};
use crate::error::LowerError;
use crate::language::PyLang;
use crate::scope::Frame;

/// Lower an expression, appending any statements it creates to `buf`, and
/// return the SSA value holding its result.
pub(crate) fn lower_expr(
    ctx: &mut BuilderStageInfo<PyLang>,
    expr: &Expr,
    frame: &mut Frame,
    buf: &mut BlockBuf,
) -> Result<SSAValue, LowerError> {
    match expr {
        Expr::Constant(c) => {
            let value = match c {
                Const::Int(n) => ArithValue::I64(*n),
                Const::Bool(b) => ArithValue::I64(*b as i64),
                Const::Float(f) => ArithValue::F64(*f),
            };
            let stmt = Constant::<ArithValue, ArithType>::new(ctx, value);
            buf.push(stmt.id);
            Ok(stmt.result.into())
        }
        Expr::Name(name) => frame
            .lookup(name)
            .ok_or_else(|| LowerError::UndefinedName(name.clone())),
        Expr::BinOp { op, lhs, rhs } => {
            let l = lower_expr(ctx, lhs, frame, buf)?;
            let r = lower_expr(ctx, rhs, frame, buf)?;
            let (id, result) = match op {
                BinOp::Add => {
                    let s = Arith::<ArithType>::op_add(ctx, l, r);
                    (s.id, s.result)
                }
                BinOp::Sub => {
                    let s = Arith::<ArithType>::op_sub(ctx, l, r);
                    (s.id, s.result)
                }
                BinOp::Mul => {
                    let s = Arith::<ArithType>::op_mul(ctx, l, r);
                    (s.id, s.result)
                }
                BinOp::Div => {
                    let s = Arith::<ArithType>::op_div(ctx, l, r);
                    (s.id, s.result)
                }
            };
            buf.push(id);
            Ok(result.into())
        }
        Expr::Compare { op, lhs, rhs } => {
            let l = lower_expr(ctx, lhs, frame, buf)?;
            let r = lower_expr(ctx, rhs, frame, buf)?;
            let (id, result) = match op {
                CmpOp::Eq => {
                    let s = Cmp::<ArithType>::op_eq(ctx, l, r);
                    (s.id, s.result)
                }
                CmpOp::Ne => {
                    let s = Cmp::<ArithType>::op_ne(ctx, l, r);
                    (s.id, s.result)
                }
                CmpOp::Lt => {
                    let s = Cmp::<ArithType>::op_lt(ctx, l, r);
                    (s.id, s.result)
                }
                CmpOp::Le => {
                    let s = Cmp::<ArithType>::op_le(ctx, l, r);
                    (s.id, s.result)
                }
                CmpOp::Gt => {
                    let s = Cmp::<ArithType>::op_gt(ctx, l, r);
                    (s.id, s.result)
                }
                CmpOp::Ge => {
                    let s = Cmp::<ArithType>::op_ge(ctx, l, r);
                    (s.id, s.result)
                }
            };
            buf.push(id);
            Ok(result.into())
        }
        Expr::Call { func, args } => {
            let mut arg_ssas = Vec::with_capacity(args.len());
            for arg in args {
                arg_ssas.push(lower_expr(ctx, arg, frame, buf)?);
            }
            // Stage-local symbol for the callee; resolves by name to the staged
            // function declared in pass 1.
            let target = ctx.symbol_table_mut().intern(func.clone());
            // Single-result kernels (the supported subset).
            let call = Call::<ArithType>::build(ctx)
                .named(target)
                .args(arg_ssas)
                .results(1)
                .insert();
            buf.push(call.id);
            let result = call
                .results
                .into_iter()
                .next()
                .ok_or_else(|| LowerError::Builder("call produced no result".into()))?;
            Ok(result.into())
        }
    }
}
