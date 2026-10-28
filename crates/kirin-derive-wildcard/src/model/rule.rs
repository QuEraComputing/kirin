//! [`RuleModel`]: the values and statements of a lowered rule, and the
//! queries the parser, the verifier and code generation ask of them.

use proc_macro2::Span;
use syn::ext::IdentExt;

use super::statement::{Binding, RuleStmt, StmtId, StmtKind, Value, ValueId};

/// A lowered `#[rewrite_rule]` function.
///
/// Values and statements are only added through its methods, so every id
/// refers to an existing entry and a statement's results always point back
/// at it. Resolving names is the parser's job.
#[derive(Debug)]
pub(crate) struct RuleModel {
    name: syn::Ident,
    values: Vec<Value>,
    /// Includes the statements nested in a `rewrite!` body.
    statements: Vec<RuleStmt>,
    /// Top level statements of the pattern's body in order as they appear
    /// in the source pattern.
    /// Excludes the statements inside a `rewrite!` block.
    body: Vec<StmtId>,
}

impl RuleModel {
    /// An empty model for the function `name`.
    pub fn new(name: syn::Ident) -> Self {
        Self {
            name: name.unraw(),
            values: Vec::new(),
            statements: Vec::new(),
            body: Vec::new(),
        }
    }

    /// The function's name, which also names the pattern.
    pub fn name(&self) -> &syn::Ident {
        &self.name
    }

    /// Get a value by its ID
    pub fn value(&self, id: ValueId) -> &Value {
        &self.values[id.0]
    }

    /// Get a value by its ID
    pub fn statement(&self, id: StmtId) -> &RuleStmt {
        &self.statements[id.0]
    }

    /// The function body's top-level statements, in source order.
    pub fn body(&self) -> &[StmtId] {
        &self.body
    }

    /// The match statements, and the final `rewrite!` statement after them.
    pub fn split_rewrite(&self) -> (&[StmtId], StmtId) {
        let (rewrite, matched) = self
            .body
            .split_last()
            .expect("the parser ends every rule with a `rewrite!`");
        (matched, *rewrite)
    }

    /// The root of the match and the body of the final `rewrite!`.
    pub fn rewrite(&self) -> (StmtId, &[StmtId]) {
        let (_, rewrite) = self.split_rewrite();
        let StmtKind::Rewrite { root, body, .. } = &self.statement(rewrite).kind else {
            unreachable!("the parser ends every rule with a `rewrite!`")
        };
        (*root, body)
    }

    /// The [`StmtKind::WalkResult`] the final `rewrite!` body ends in, if any.
    pub fn walk_result(&self) -> Option<StmtId> {
        let (_, rewrite) = self.split_rewrite();
        let StmtKind::Rewrite { walk_result, .. } = &self.statement(rewrite).kind else {
            unreachable!("the parser ends every rule with a `rewrite!`")
        };
        *walk_result
    }

    /// Add a hole statement for a name used without a `let` binding it, or
    /// for `_` as an argument (`name` is then `None`). Its one result is the
    /// hole's value.
    pub fn add_hole(&mut self, name: Option<syn::Ident>, span: Span) -> StmtId {
        self.add_statement(span, StmtKind::Hole, [(name, span)])
    }

    /// Whether `id` is a hole's value rather than the result of a statement
    /// the rule writes.
    pub fn is_hole(&self, id: ValueId) -> bool {
        matches!(self.statement(self.value(id).stmt).kind, StmtKind::Hole)
    }

    /// Add a statement together with the values its `let` binds, given as
    /// `(name, span)` in pattern order. A statement without a `let` has no
    /// results.
    pub fn add_statement(
        &mut self,
        span: Span,
        kind: StmtKind,
        values: impl IntoIterator<Item = Binding>,
    ) -> StmtId {
        let stmt = StmtId(self.statements.len());
        let values = values
            .into_iter()
            .map(|(name, span)| self.push_value(Value { name, span, stmt }))
            .collect();
        self.statements.push(RuleStmt {
            span,
            kind,
            results: values,
        });
        stmt
    }

    /// Append `stmt` to the function body's top-level statements.
    pub fn push_body(&mut self, stmt: StmtId) {
        self.body.push(stmt);
    }

    /// Append `value` to the collection of this rule's values.
    fn push_value(&mut self, value: Value) -> ValueId {
        self.values.push(value);
        ValueId(self.values.len() - 1)
    }

    /// Whether `id` corresponds to the result of an `op![..]` in the match,
    /// rather than a hole, a native result, or an op a `rewrite!` body builds.
    pub fn is_matched_op_result(&self, id: ValueId) -> bool {
        self.is_matched_op(self.value(id).stmt)
    }

    /// Whether `stmt` corresponds to an `op![..]` in the match, rather than a
    /// hole, a native call, or an op a `rewrite!` body builds.
    pub fn is_matched_op(&self, stmt: StmtId) -> bool {
        self.body.contains(&stmt) && matches!(self.statement(stmt).kind, StmtKind::Op { .. })
    }

    /// Whether `id` is the result of a native function called in the match.
    /// A value computed from the match cannot build anything in the IR since
    /// the rule may still fail to apply and we only want to modify the IR if
    /// the rule does apply.
    pub fn is_matched_native_result(&self, id: ValueId) -> bool {
        self.in_match(id) && matches!(self.defining(id), StmtKind::NativeCompute { .. })
    }

    /// Whether `id` is the result of a native function called in the
    /// `rewrite!` body, which runs only once the rule applies.
    pub fn is_rewrite_native_result(&self, id: ValueId) -> bool {
        !self.in_match(id) && matches!(self.defining(id), StmtKind::NativeCompute { .. })
    }

    /// Whether `id` is defined by one of the body's statements.
    fn in_match(&self, id: ValueId) -> bool {
        self.body.contains(&self.value(id).stmt)
    }

    /// The kind of the statement defining `id`.
    pub(super) fn defining(&self, id: ValueId) -> &StmtKind {
        &self.statement(self.value(id).stmt).kind
    }
}
