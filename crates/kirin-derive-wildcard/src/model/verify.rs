//! [`RuleModel::verify`]: the checks that need the whole rule, run once the
//! parser has built it.

use std::collections::HashSet;

use super::RuleModel;
use super::statement::{StmtId, StmtKind, Use};

impl RuleModel {
    /// Check the whole rule, reporting every mistake at once, combined into
    /// one error.
    ///
    /// The match must be exactly what matching needs: only the producers of
    /// the rewrite's root, traversed through operands and stopping at holes.
    /// Every op in the match must therefore feed the root.
    ///
    /// The rewrite's body must also use nothing an earlier action of it
    /// removed (see [`Self::check_liveness`]).
    pub(crate) fn verify(&self) -> syn::Result<()> {
        let (root, body) = self.rewrite();

        // Track the ops that feed the root.
        let mut matched = HashSet::new();
        self.visit_producers(root, &mut matched)?;

        let mut errors: Option<syn::Error> = None;
        // Closure used to combine errors so that they are all reported together.
        let mut report = |error: syn::Error| match &mut errors {
            Some(all) => all.combine(error),
            None => errors = Some(error),
        };

        // Report the ops that do not feed the root and find any holes that
        // are only read by them.
        let mut unmatched_args = HashSet::new();
        for &id in self.body() {
            let stmt = self.statement(id);
            if let StmtKind::Op { args, .. } = &stmt.kind
                && !matched.contains(&id)
            {
                unmatched_args.extend(args.iter().map(|arg| arg.value));
                report(syn::Error::new(
                    stmt.span,
                    "this `op![..]` does not feed the root of the rewrite, so it is never matched",
                ));
            }
        }

        // Report the holes no op reads at all.
        for &id in self.body() {
            let stmt = self.statement(id);
            let hole = stmt.results.first().copied();
            if let (StmtKind::Hole, Some(hole)) = (&stmt.kind, hole)
                && !matched.contains(&id)
                && !unmatched_args.contains(&hole)
            {
                let value = self.value(hole);
                let name = value.display_name();
                report(syn::Error::new(
                    value.span,
                    format!(
                        "the hole `{name}` is not an argument of any `op![..]`, so matching will never bind it"
                    ),
                ));
            }
        }

        self.check_liveness(body, &mut report);

        errors.map_or(Ok(()), Err)
    }

    /// Add `stmt_id` to `matched`, recursively visit the statement's operands
    fn visit_producers(&self, stmt_id: StmtId, matched: &mut HashSet<StmtId>) -> syn::Result<()> {
        // End the traversal if previously visited
        if !matched.insert(stmt_id) {
            return Ok(());
        }

        let stmt = self.statement(stmt_id);
        let StmtKind::Op { args, .. } = &stmt.kind else {
            return Ok(());
        };

        for arg in args {
            if matches!(self.defining(arg.value), StmtKind::NativeCompute { .. }) {
                return Err(syn::Error::new(
                    stmt.span,
                    "a matched `op![..]` cannot take a native function's result as an operand",
                ));
            }
            self.visit_producers(self.value(arg.value).stmt, matched)?;
        }
        Ok(())
    }

    /// Report every use-after-free in the rewrite's `body`, i.e. any reference
    /// to an op or result that has been replaced or erased.
    fn check_liveness(&self, body: &[StmtId], report: &mut impl FnMut(syn::Error)) {
        let mut replaced = HashSet::new();
        let mut erased = HashSet::new();

        let check = |value: &Use, replaced: &HashSet<_>, erased: &HashSet<_>| {
            let info = self.value(value.value);
            let name = info.display_name();
            let message = if erased.contains(&info.stmt) {
                format!(
                    "`{name}` is a result of an op erased by `erase!` above and cannot be used afterwards"
                )
            } else if replaced.contains(&value.value) {
                format!("`{name}` was redirected by `replace!` above and cannot be used afterwards")
            } else {
                return None;
            };
            Some(syn::Error::new(value.span, message))
        };

        for &id in body {
            match &self.statement(id).kind {
                StmtKind::Op { args, .. } | StmtKind::NativeCompute { args, .. } => {
                    args.iter()
                        .filter_map(|arg| check(arg, &replaced, &erased))
                        .for_each(&mut *report);
                }
                StmtKind::Replace {
                    matched,
                    replacement,
                } => {
                    check(matched, &replaced, &erased).map_or_default(&mut *report);
                    check(replacement, &replaced, &erased).map_or_default(&mut *report);

                    replaced.insert(matched.value);
                }
                StmtKind::Erase { stmt: op } => {
                    if !erased.insert(*op) {
                        report(syn::Error::new(
                            self.statement(id).span,
                            "the op this refers to was already erased by `erase!` above",
                        ));
                    }
                }
                StmtKind::Hole
                | StmtKind::Constraint { .. }
                | StmtKind::Rewrite { .. }
                | StmtKind::WalkResult(_) => {}
            }
        }
    }
}
