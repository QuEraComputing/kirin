//! Convert CPython `ast` objects (received across the PyO3 boundary) into the
//! pure-Rust `PyAst` mirror consumed by `kirin-lower`.
//!
//! Dispatch is on each node's Python class name (`type(node).__name__`).

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyList};

use crate::ast::{Arg, BinOp, CmpOp, Const, Expr, FunctionDef, Module, PyType, RangeCall, Stmt};

fn unsupported(msg: impl AsRef<str>) -> PyErr {
    PyValueError::new_err(format!("unsupported Python: {}", msg.as_ref()))
}

/// `type(node).__name__`.
fn type_name(node: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(node.get_type().name()?.to_string())
}

/// Convert an `ast.Module` into a [`Module`] (only top-level `def`s allowed).
pub fn module(node: &Bound<'_, PyAny>) -> PyResult<Module> {
    let body = node.getattr("body")?;
    let body = body.cast::<PyList>()?;
    let mut funcs = Vec::new();
    for item in body.iter() {
        match type_name(&item)?.as_str() {
            "FunctionDef" => funcs.push(function(&item)?),
            other => {
                return Err(unsupported(format!(
                    "top-level `{other}` (only `def` is supported)"
                )));
            }
        }
    }
    Ok(Module { body: funcs })
}

fn function(node: &Bound<'_, PyAny>) -> PyResult<FunctionDef> {
    let name = node.getattr("name")?.extract::<String>()?;
    let arguments = node.getattr("args")?;
    let positional = arguments.getattr("args")?;
    let positional = positional.cast::<PyList>()?;
    let mut args = Vec::new();
    for a in positional.iter() {
        let arg_name = a.getattr("arg")?.extract::<String>()?;
        let annotation = annotation(&a.getattr("annotation")?)?;
        args.push(Arg {
            name: arg_name,
            annotation,
        });
    }
    let returns = annotation(&node.getattr("returns")?)?;
    let body = stmts(&node.getattr("body")?)?;
    Ok(FunctionDef {
        name,
        args,
        returns,
        body,
    })
}

/// A type annotation is an `ast.Name` whose `id` is `int`/`bool`/`float`, or
/// `None`. Unknown annotations default to `None` (treated as `int`/`i64`).
fn annotation(node: &Bound<'_, PyAny>) -> PyResult<Option<PyType>> {
    if node.is_none() {
        return Ok(None);
    }
    let id = node
        .getattr("id")
        .and_then(|n| n.extract::<String>())
        .unwrap_or_default();
    Ok(match id.as_str() {
        "int" => Some(PyType::Int),
        "bool" => Some(PyType::Bool),
        "float" => Some(PyType::Float),
        _ => None,
    })
}

fn stmts(list: &Bound<'_, PyAny>) -> PyResult<Vec<Stmt>> {
    let list = list.cast::<PyList>()?;
    let mut out = Vec::new();
    for item in list.iter() {
        // `pass` produces no IR.
        if type_name(&item)? == "Pass" {
            continue;
        }
        out.push(stmt(&item)?);
    }
    Ok(out)
}

fn stmt(node: &Bound<'_, PyAny>) -> PyResult<Stmt> {
    match type_name(node)?.as_str() {
        "Assign" => {
            let targets = node.getattr("targets")?;
            let targets = targets.cast::<PyList>()?;
            if targets.len() != 1 {
                return Err(unsupported("multiple/tuple assignment targets"));
            }
            let target = targets
                .get_item(0)?
                .getattr("id")
                .and_then(|n| n.extract::<String>())
                .map_err(|_| unsupported("only simple `name = ...` assignment"))?;
            let value = expr(&node.getattr("value")?)?;
            Ok(Stmt::Assign { target, value })
        }
        "AnnAssign" => {
            let target = node
                .getattr("target")?
                .getattr("id")
                .and_then(|n| n.extract::<String>())
                .map_err(|_| unsupported("only simple `name: T = ...` assignment"))?;
            let value = node.getattr("value")?;
            if value.is_none() {
                return Err(unsupported("annotation-only declaration without a value"));
            }
            let value = expr(&value)?;
            Ok(Stmt::Assign { target, value })
        }
        "Return" => {
            let value = node.getattr("value")?;
            let value = if value.is_none() {
                None
            } else {
                Some(expr(&value)?)
            };
            Ok(Stmt::Return { value })
        }
        "If" => Ok(Stmt::If {
            test: expr(&node.getattr("test")?)?,
            body: stmts(&node.getattr("body")?)?,
            orelse: stmts(&node.getattr("orelse")?)?,
        }),
        "For" => {
            if !node.getattr("orelse")?.cast::<PyList>()?.is_empty() {
                return Err(unsupported("`for ... else` clause"));
            }
            let target = node
                .getattr("target")?
                .getattr("id")
                .and_then(|n| n.extract::<String>())
                .map_err(|_| unsupported("only simple `for name in ...` loops"))?;
            let iter = range_call(&node.getattr("iter")?)?;
            let body = stmts(&node.getattr("body")?)?;
            Ok(Stmt::For { target, iter, body })
        }
        "Expr" => Ok(Stmt::Expr(expr(&node.getattr("value")?)?)),
        other => Err(unsupported(format!("statement `{other}`"))),
    }
}

/// A for-loop iterable must be a `range(lo[, hi[, step]])` call.
fn range_call(node: &Bound<'_, PyAny>) -> PyResult<RangeCall> {
    let is_range = type_name(node)? == "Call"
        && node
            .getattr("func")?
            .getattr("id")
            .and_then(|n| n.extract::<String>())
            .map(|n| n == "range")
            .unwrap_or(false);
    if !is_range {
        return Err(unsupported("for-loop iterable must be `range(...)`"));
    }
    let args = node.getattr("args")?;
    let args = args.cast::<PyList>()?;
    let nth = |i: usize| expr(&args.get_item(i).unwrap());
    match args.len() {
        1 => Ok(RangeCall {
            lo: Expr::Constant(Const::Int(0)),
            hi: nth(0)?,
            step: None,
        }),
        2 => Ok(RangeCall {
            lo: nth(0)?,
            hi: nth(1)?,
            step: None,
        }),
        3 => Ok(RangeCall {
            lo: nth(0)?,
            hi: nth(1)?,
            step: Some(nth(2)?),
        }),
        _ => Err(unsupported("range() expects 1-3 arguments")),
    }
}

fn expr(node: &Bound<'_, PyAny>) -> PyResult<Expr> {
    match type_name(node)?.as_str() {
        "Constant" => {
            let value = node.getattr("value")?;
            // `bool` is a subclass of `int`, so check it first.
            if value.is_instance_of::<PyBool>() {
                Ok(Expr::Constant(Const::Bool(value.extract::<bool>()?)))
            } else if let Ok(i) = value.extract::<i64>() {
                Ok(Expr::Constant(Const::Int(i)))
            } else if let Ok(f) = value.extract::<f64>() {
                Ok(Expr::Constant(Const::Float(f)))
            } else {
                Err(unsupported("only int/float/bool literals"))
            }
        }
        "Name" => Ok(Expr::Name(node.getattr("id")?.extract::<String>()?)),
        "BinOp" => {
            let op = match type_name(&node.getattr("op")?)?.as_str() {
                "Add" => BinOp::Add,
                "Sub" => BinOp::Sub,
                "Mult" => BinOp::Mul,
                "Div" => BinOp::Div,
                other => return Err(unsupported(format!("binary operator `{other}`"))),
            };
            Ok(Expr::BinOp {
                op,
                lhs: Box::new(expr(&node.getattr("left")?)?),
                rhs: Box::new(expr(&node.getattr("right")?)?),
            })
        }
        "Compare" => {
            let ops = node.getattr("ops")?;
            let ops = ops.cast::<PyList>()?;
            let comparators = node.getattr("comparators")?;
            let comparators = comparators.cast::<PyList>()?;
            if ops.len() != 1 {
                return Err(unsupported("chained comparisons (e.g. `a < b < c`)"));
            }
            let op = match type_name(&ops.get_item(0)?)?.as_str() {
                "Eq" => CmpOp::Eq,
                "NotEq" => CmpOp::Ne,
                "Lt" => CmpOp::Lt,
                "LtE" => CmpOp::Le,
                "Gt" => CmpOp::Gt,
                "GtE" => CmpOp::Ge,
                other => return Err(unsupported(format!("comparison operator `{other}`"))),
            };
            Ok(Expr::Compare {
                op,
                lhs: Box::new(expr(&node.getattr("left")?)?),
                rhs: Box::new(expr(&comparators.get_item(0)?)?),
            })
        }
        "Call" => {
            let func = node
                .getattr("func")?
                .getattr("id")
                .and_then(|n| n.extract::<String>())
                .map_err(|_| unsupported("only direct calls to named kernels"))?;
            let args = node.getattr("args")?;
            let args = args.cast::<PyList>()?;
            let mut lowered = Vec::new();
            for a in args.iter() {
                lowered.push(expr(&a)?);
            }
            Ok(Expr::Call {
                func,
                args: lowered,
            })
        }
        other => Err(unsupported(format!("expression `{other}`"))),
    }
}
