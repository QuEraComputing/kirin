//! What the names of a rule refer to while it is parsed.

use std::collections::HashSet;

use proc_macro2::Span;
use syn::ext::IdentExt;

use super::RuleParser;
use crate::model::{Binding, StmtId, StmtKind, ValueId};

/// What a name of a rule refers to.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Name {
    /// A value: a hole, a result of an op, or a result of a native call.
    Value(ValueId),
    /// An op itself, named by `let s @ (..) = op![..]`.
    Op(StmtId),
}

impl RuleParser {
    /// The value `name` refers to: its `let` binding, or else the value of a
    /// hole statement. If it is the first use of the name as a hole, it is
    /// created, added to the body and shared by every later use.
    pub(super) fn resolve_value(&mut self, name: &syn::Ident) -> syn::Result<ValueId> {
        match self.names.get(&name.unraw().to_string()).copied() {
            Some(Name::Value(id)) => Ok(id),
            Some(Name::Op(_)) => Err(syn::Error::new_spanned(
                name,
                format!("`{name}` names an op, not a value: use one of its results"),
            )),
            None => {
                let id = self.add_hole(Some(name.clone()), name.span());
                self.names.insert(name.unraw().to_string(), Name::Value(id));
                Ok(id)
            }
        }
    }

    /// The op! statement that `target` refers to. Called when parsing the args
    /// of `rewrite!` and `erase!`. `target` may be the statement bound with `@`
    /// or a statement's result. In the latter case, the statement producing
    /// the result is returned.
    ///
    /// Fails with message `misuse` if `target` is not such a name, e.g. a hole.
    pub(super) fn resolve_op(&self, target: &syn::Expr, misuse: &str) -> syn::Result<StmtId> {
        // Defer creating error to only when it is needed
        let misuse = || syn::Error::new_spanned(target, misuse);

        let syn::Expr::Path(path) = target else {
            return Err(misuse());
        };
        // Name should only be a simple ident
        let Some(name) = path.path.get_ident() else {
            return Err(misuse());
        };

        let op = match self.names.get(&name.unraw().to_string()) {
            Some(&Name::Op(op)) => op,
            Some(&Name::Value(id)) => self.model.value(id).stmt,
            None => return Err(misuse()),
        };
        // Should not resolve for ops inside the rewrite block
        if !self.model.is_matched_op(op) {
            return Err(misuse());
        }
        Ok(op)
    }

    /// Add a hole statement to the body and return its value: named for a
    /// name used without a `let`, or anonymous (`None`) for `_` as an
    /// argument of `op![..]`.
    pub(super) fn add_hole(&mut self, name: Option<syn::Ident>, span: Span) -> ValueId {
        let hole = self.model.add_hole(name, span);
        self.model.push_body(hole);
        self.model.statement(hole).results[0]
    }

    /// Add a statement to the model and bind the names its `let` introduces.
    /// Fails, adding nothing, if a name is already bound, was used as a hole
    /// before this `let`, or appears twice in the pattern.
    pub(super) fn add_statement(
        &mut self,
        span: Span,
        kind: StmtKind,
        values: Vec<Binding>,
    ) -> syn::Result<StmtId> {
        self.add_named_statement(span, kind, None, values)
    }

    /// [`Self::add_statement`], but also giving the statement itself `op_name`
    /// as `let s @ (q, r) = op![..]` allows.
    pub(super) fn add_named_statement(
        &mut self,
        span: Span,
        kind: StmtKind,
        op_name: Option<syn::Ident>,
        values: Vec<Binding>,
    ) -> syn::Result<StmtId> {
        let names = op_name
            .iter()
            .chain(values.iter().flat_map(|(name, _)| name));
        let mut in_pattern = HashSet::new();
        for name in names {
            let key = name.unraw().to_string();

            // A hole was created at an earlier use of the name, so it cannot
            // be bound now.
            if let Some(&Name::Value(id)) = self.names.get(&key)
                && self.model.is_hole(id)
            {
                return Err(syn::Error::new_spanned(
                    name,
                    format!("`{name}` is used before the `let` that binds it"),
                ));
            }

            // Bound by an earlier `let`, or twice in this pattern
            // (`let (x, x) = ..`, `let s @ s = ..`).
            if self.names.contains_key(&key) || !in_pattern.insert(key) {
                return Err(syn::Error::new_spanned(
                    name,
                    format!("duplicate identifier `{name}` in `#[rewrite_rule]` function"),
                ));
            }
        }

        let stmt = self.model.add_statement(span, kind, values);
        if let Some(name) = op_name {
            self.names.insert(name.unraw().to_string(), Name::Op(stmt));
        }
        for &id in &self.model.statement(stmt).results {
            if let Some(name) = &self.model.value(id).name {
                self.names.insert(name.unraw().to_string(), Name::Value(id));
            }
        }
        Ok(stmt)
    }
}
