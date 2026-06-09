//! Shared `PyAst` fixtures for the kirin-lower integration tests.
//!
//! Lives in a `common/` subdirectory module so cargo does not treat it as its
//! own test binary; each `tests/*.rs` pulls it in via `mod common;`.
#![allow(dead_code)]

use kirin_py_frontend::ast::*;

pub fn int_arg(name: &str) -> Arg {
    Arg {
        name: name.into(),
        annotation: Some(PyType::Int),
    }
}

pub fn name(n: &str) -> Expr {
    Expr::Name(n.into())
}

pub fn binop(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// `def add(a: int, b: int) -> int: return a + b`
pub fn add_module() -> Module {
    Module {
        body: vec![FunctionDef {
            name: "add".into(),
            args: vec![int_arg("a"), int_arg("b")],
            returns: Some(PyType::Int),
            body: vec![Stmt::Return {
                value: Some(binop(BinOp::Add, name("a"), name("b"))),
            }],
        }],
    }
}

/// A function exercising all four arithmetic ops through local assignments.
///
/// ```text
/// def f(x: int, y: int) -> int:
///     a = x + y
///     b = a - x
///     c = b * y
///     d = c / x
///     return d
/// ```
pub fn arith_chain_module() -> Module {
    Module {
        body: vec![FunctionDef {
            name: "f".into(),
            args: vec![int_arg("x"), int_arg("y")],
            returns: Some(PyType::Int),
            body: vec![
                Stmt::Assign {
                    target: "a".into(),
                    value: binop(BinOp::Add, name("x"), name("y")),
                },
                Stmt::Assign {
                    target: "b".into(),
                    value: binop(BinOp::Sub, name("a"), name("x")),
                },
                Stmt::Assign {
                    target: "c".into(),
                    value: binop(BinOp::Mul, name("b"), name("y")),
                },
                Stmt::Assign {
                    target: "d".into(),
                    value: binop(BinOp::Div, name("c"), name("x")),
                },
                Stmt::Return {
                    value: Some(name("d")),
                },
            ],
        }],
    }
}

/// `def pick(c, a, b): if c > 0: r = a + b else: r = a - b; return r`
pub fn pick_module() -> Module {
    let cond = Expr::Compare {
        op: CmpOp::Gt,
        lhs: Box::new(name("c")),
        rhs: Box::new(Expr::Constant(Const::Int(0))),
    };
    let assign = |op| Stmt::Assign {
        target: "r".into(),
        value: binop(op, name("a"), name("b")),
    };
    Module {
        body: vec![FunctionDef {
            name: "pick".into(),
            args: vec![int_arg("c"), int_arg("a"), int_arg("b")],
            returns: Some(PyType::Int),
            body: vec![
                Stmt::If {
                    test: cond,
                    body: vec![assign(BinOp::Add)],
                    orelse: vec![assign(BinOp::Sub)],
                },
                Stmt::Return {
                    value: Some(name("r")),
                },
            ],
        }],
    }
}

/// `def sum_to(n): s = 0; for i in range(0, n): s = s + i; return s`
pub fn sum_to_module() -> Module {
    Module {
        body: vec![FunctionDef {
            name: "sum_to".into(),
            args: vec![int_arg("n")],
            returns: Some(PyType::Int),
            body: vec![
                Stmt::Assign {
                    target: "s".into(),
                    value: Expr::Constant(Const::Int(0)),
                },
                Stmt::For {
                    target: "i".into(),
                    iter: RangeCall {
                        lo: Expr::Constant(Const::Int(0)),
                        hi: name("n"),
                        step: None,
                    },
                    body: vec![Stmt::Assign {
                        target: "s".into(),
                        value: binop(BinOp::Add, name("s"), name("i")),
                    }],
                },
                Stmt::Return {
                    value: Some(name("s")),
                },
            ],
        }],
    }
}

/// Two kernels where `main` calls `helper`.
pub fn call_module() -> Module {
    Module {
        body: vec![
            FunctionDef {
                name: "helper".into(),
                args: vec![int_arg("x")],
                returns: Some(PyType::Int),
                body: vec![Stmt::Return {
                    value: Some(binop(BinOp::Add, name("x"), name("x"))),
                }],
            },
            FunctionDef {
                name: "main".into(),
                args: vec![int_arg("y")],
                returns: Some(PyType::Int),
                body: vec![
                    Stmt::Assign {
                        target: "z".into(),
                        value: Expr::Call {
                            func: "helper".into(),
                            args: vec![name("y")],
                        },
                    },
                    Stmt::Return {
                        value: Some(name("z")),
                    },
                ],
            },
        ],
    }
}

/// `def factorial(n): if n <= 1: r = 1 else: r = n * factorial(n - 1); return r`
/// Recursion expressed via if/else value-merge (no early return).
pub fn factorial_module() -> Module {
    Module {
        body: vec![FunctionDef {
            name: "factorial".into(),
            args: vec![int_arg("n")],
            returns: Some(PyType::Int),
            body: vec![
                Stmt::If {
                    test: Expr::Compare {
                        op: CmpOp::Le,
                        lhs: Box::new(name("n")),
                        rhs: Box::new(Expr::Constant(Const::Int(1))),
                    },
                    body: vec![Stmt::Assign {
                        target: "r".into(),
                        value: Expr::Constant(Const::Int(1)),
                    }],
                    orelse: vec![Stmt::Assign {
                        target: "r".into(),
                        value: binop(
                            BinOp::Mul,
                            name("n"),
                            Expr::Call {
                                func: "factorial".into(),
                                args: vec![binop(
                                    BinOp::Sub,
                                    name("n"),
                                    Expr::Constant(Const::Int(1)),
                                )],
                            },
                        ),
                    }],
                },
                Stmt::Return {
                    value: Some(name("r")),
                },
            ],
        }],
    }
}

/// Every fixture module — used by the roundtrip test.
pub fn all_modules() -> Vec<Module> {
    vec![
        add_module(),
        arith_chain_module(),
        pick_module(),
        sum_to_module(),
        call_module(),
        factorial_module(),
    ]
}
