//! A Rust mirror of the supported Python AST subset.
//!
//! The PyO3 bridge (`kirin-py`) converts CPython `ast` nodes into these types;
//! the lowering visitor consumes them. Keeping this as a plain Rust enum lets
//! the whole lowering pipeline be unit-tested without a Python interpreter.

/// A module: a flat list of top-level function definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub body: Vec<FunctionDef>,
}

/// `def name(args) -> returns: body`.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: String,
    pub args: Vec<Arg>,
    pub returns: Option<PyType>,
    pub body: Vec<Stmt>,
}

/// A single positional parameter with an optional type annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: String,
    pub annotation: Option<PyType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `target = value` (single `Name` target only).
    Assign { target: String, value: Expr },
    /// `return value` (or bare `return`).
    Return { value: Option<Expr> },
    /// `if test: body else: orelse`.
    If {
        test: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    /// `for target in range(..): body`.
    For {
        target: String,
        iter: RangeCall,
        body: Vec<Stmt>,
    },
    /// A bare expression statement (e.g. a call whose result is discarded).
    Expr(Expr),
}

/// `range(lo, hi[, step])`.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeCall {
    pub lo: Expr,
    pub hi: Expr,
    pub step: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Constant(Const),
    Name(String),
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// A single comparison `lhs <op> rhs`.
    Compare {
        op: CmpOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// `func(args)` — a call to another top-level kernel.
    Call {
        func: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyType {
    Int,
    Bool,
    Float,
}
