//! The values and statements a [`RuleModel`](super::RuleModel) is made of.

use proc_macro2::Span;

/// A value a `let` binds: `(name, span)`, with `None`` name for `_`.
pub(crate) type Binding = (Option<syn::Ident>, Span);

/// A value in a [`RuleModel`](super::RuleModel); only the model creates
/// these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ValueId(pub(super) usize);

impl ValueId {
    /// The value's position in its model, unique among the model's values.
    pub fn index(self) -> usize {
        self.0
    }
}

/// A statement in a [`RuleModel`](super::RuleModel); only the model creates
/// these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StmtId(pub(super) usize);

impl StmtId {
    /// The statement's position in its model, unique among the model's
    /// statements.
    pub fn index(self) -> usize {
        self.0
    }
}

/// One place the rule names a value: the value, and the span of the tokens
/// naming it there, so an error about this use points at it rather than at
/// the value's definition.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Use {
    pub value: ValueId,
    pub span: Span,
}

/// A value used in the rule: one of `stmt`'s results.
#[derive(Debug)]
pub(crate) struct Value {
    /// `None` for an anonymous `_`.
    pub name: Option<syn::Ident>,
    /// Where the value is defined: its name in the `let`, or, for a hole,
    /// its first use.
    pub span: Span,
    /// The statement defining it: the `let` binding it, or, for a hole, the
    /// hole statement.
    pub stmt: StmtId,
}

impl Value {
    /// The value's name as the rule writes it, `_` for an anonymous one.
    pub fn display_name(&self) -> String {
        self.name
            .as_ref()
            .map_or_else(|| "_".to_string(), ToString::to_string)
    }
}

/// One statement of the rule.
#[derive(Debug)]
pub(crate) struct RuleStmt {
    pub span: Span,
    pub kind: StmtKind,
    /// The values its `let` binds, in pattern order.
    pub results: Vec<ValueId>,
}

/// What a [`RuleStmt`] does.
#[derive(Debug)]
pub(crate) enum StmtKind {
    /// A name used without a `let` binding it, or `_` as an argument. Added at
    /// the name's first use and placed in the body before the statement
    /// using it.
    Hole,
    /// `let r = op![Arith::mul(x, c)];`: the matching of a statement named as
    /// `Dialect::op`, applied to `args`. `let s @ (q, rem) = ..`
    /// also names the statement itself `s`; that name only lives in the
    /// parser's name table and is mainly used to bind zero-result statements.
    Op { op: syn::Path, args: Vec<Use> },
    /// `let log_val = log2(val);`: a native function computing values.
    NativeCompute { func: syn::Path, args: Vec<Use> },
    /// Requires `success` to hold. `is_power_of_two(val);` becomes a
    /// [`StmtKind::NativeCompute`] whose anonymous result is `success`,
    /// followed by this.
    Constraint { success: ValueId },
    /// `rewrite!(r, { .. });`: the replacement for the match rooted at the
    /// statement `root`. In the source, `root` may be named directly via a
    /// `let s @ (..)` binding, or by one of its results.
    Rewrite { root: StmtId, body: Vec<StmtId> },
    /// `replace!(r, new_r);`: redirect every use of `matched` to `replacement`.
    Replace { matched: Use, replacement: Use },
    /// `erase!(r);`: erase the matched `stmt`, named directly in the source
    /// rule or by one of its results.
    Erase { stmt: StmtId },
}
