use kirin_derive_toolkit::misc::to_snake_case;
use proc_macro2::Span;
use syn::{Stmt, Token, ext::IdentExt, punctuated::Punctuated, spanned::Spanned};

use super::{RuleParser, rule_parser::MISSING_REWRITE};
use crate::model::{Binding, StmtId, StmtKind, Use, WalkResult};

const OP_PATH: &str = "ops in `op![..]` must name their dialect, e.g. `op![Arith::add(x, y)]`";

/// The macros naming a fixed walk result, the tail expression of a
/// `rewrite!` body.
const WALK_MACROS: [&str; 3] = ["advance", "skip", "interrupt"];

impl RuleParser {
    /// Parse a statement of the match: a `let` binding or a constraint call,
    /// returning the statements it adds in order. Rewrite actions are only
    /// allowed as the last statement of the function.
    pub(super) fn parse_statement(&mut self, stmt: &Stmt) -> syn::Result<Vec<StmtId>> {
        match stmt {
            Stmt::Local(local) => Ok(vec![self.parse_local(local)?]),
            // `syn` parses a macro call with a semicolon as `Stmt::Macro` and
            // one without as an expression.
            Stmt::Macro(syn::StmtMacro { mac, .. })
            | Stmt::Expr(syn::Expr::Macro(syn::ExprMacro { mac, .. }), _) => {
                Err(syn::Error::new_spanned(
                    mac,
                    "only `let` bindings and constraint calls can come before the rewrite in `#[rewrite_rule]` functions",
                ))
            }
            // A call whose result is not bound is a constraint.
            Stmt::Expr(syn::Expr::Call(call), Some(_)) => {
                Ok(self.parse_constraint(call, stmt.span())?.to_vec())
            }
            Stmt::Expr(expr, _) => Err(syn::Error::new_spanned(
                expr,
                "only constraint function calls are allowed as expressions in `#[rewrite_rule]` functions",
            )),
            Stmt::Item(_) => Err(syn::Error::new_spanned(
                stmt,
                "items are not allowed in `#[rewrite_rule]` functions",
            )),
        }
    }

    /// Parse a constraint call `is_power_of_two(val);` into two statements.
    /// It becomes: a `NativeCompute` whose one anonymous result says whether
    /// the constraint holds and a `Constraint` which takes that result.
    /// Both, and the result, carry the call's `span`.
    pub(super) fn parse_constraint(
        &mut self,
        call: &syn::ExprCall,
        span: Span,
    ) -> syn::Result<[StmtId; 2]> {
        let (func, args) = self.parse_native_call(call)?;
        let compute = self.add_statement(
            span,
            StmtKind::NativeCompute { func, args },
            vec![(None, span)],
        )?;
        let success = self.model().statement(compute).results[0];
        let constraint = self.add_statement(span, StmtKind::Constraint { success }, vec![])?;
        Ok([compute, constraint])
    }

    /// Parse `let pattern = init;`, where `init` is a native call or `op![..]`.
    pub(super) fn parse_local(&mut self, local: &syn::Local) -> syn::Result<StmtId> {
        let Some(init) = &local.init else {
            return Err(syn::Error::new_spanned(
                &local.pat,
                "all local bindings in `#[rewrite_rule]` functions must have an initializer",
            ));
        };
        if init.diverge.is_some() {
            return Err(syn::Error::new_spanned(
                local,
                "diverging expressions are not allowed in `#[rewrite_rule]` functions",
            ));
        }

        // The initializer is resolved before the pattern binds its names, so
        // `let x = f(x)` uses `x` before binding it.
        let (kind, op_name, values) = match init.expr.as_ref() {
            syn::Expr::Call(call) => {
                let (func, args) = self.parse_native_call(call)?;
                let values = Self::parse_pat(&local.pat)?;
                (StmtKind::NativeCompute { func, args }, None, values)
            }
            syn::Expr::Macro(syn::ExprMacro { mac, .. }) => {
                let kind = self.parse_op(mac)?;
                let (op_name, values) = Self::parse_op_pat(&local.pat)?;
                (kind, op_name, values)
            }
            expr => {
                return Err(syn::Error::new_spanned(
                    expr,
                    "only function and macro calls are allowed as initializers in `#[rewrite_rule]` functions",
                ));
            }
        };
        self.add_named_statement(local.span(), kind, op_name, values)
    }

    /// The `let` pattern of an `op![..]`: `s @ (q, r)` names the op `s` and
    /// binds its results with `(q, r)` (`s @ ()` for an op without results);
    /// any other pattern only binds the results.
    pub(super) fn parse_op_pat(pat: &syn::Pat) -> syn::Result<(Option<syn::Ident>, Vec<Binding>)> {
        if let syn::Pat::Ident(ident) = pat
            && let Some((_, results)) = &ident.subpat
        {
            let name = Self::parse_ident(ident)?;
            return Ok((Some(name), Self::parse_pat(results)?));
        }
        Ok((None, Self::parse_pat(pat)?))
    }

    /// The values a `let` pattern binds, as `(name, span)` in pattern order. Names are bound later, by [`Self::add_statement`].
    pub(super) fn parse_pat(pat: &syn::Pat) -> syn::Result<Vec<Binding>> {
        let mut values = vec![];
        match pat {
            syn::Pat::Ident(ident) => {
                if let Some((at, _)) = &ident.subpat {
                    return Err(syn::Error::new_spanned(
                        at,
                        "`@` can only name the statement of an `op![..]`, e.g. `let s @ (q, r) = op![..]`",
                    ));
                }
                let ident = Self::parse_ident(ident)?;
                let span = ident.span();
                values.push((Some(ident), span));
            }
            syn::Pat::Tuple(tuple) => {
                for elem in tuple.elems.iter() {
                    values.extend(Self::parse_pat(elem)?);
                }
            }
            // TODO: accept type annotations
            syn::Pat::Type(pat_type) => {
                return Err(syn::Error::new_spanned(
                    &pat_type.ty,
                    "type annotations are not supported in `#[rewrite_rule]` functions",
                ));
            }
            syn::Pat::Wild(wild) => values.push((None, wild.underscore_token.span)),
            _ => {
                return Err(syn::Error::new_spanned(
                    pat,
                    "only identifier patterns are allowed in `#[rewrite_rule]` functions",
                ));
            }
        }
        Ok(values)
    }

    pub(super) fn parse_ident(ident: &syn::PatIdent) -> syn::Result<syn::Ident> {
        if ident.mutability.is_some() {
            return Err(syn::Error::new_spanned(
                ident,
                "mutable identifiers are not allowed in `#[rewrite_rule]` functions",
            ));
        }
        if ident.by_ref.is_some() {
            return Err(syn::Error::new_spanned(
                ident,
                "by-ref identifiers are not allowed in `#[rewrite_rule]` functions",
            ));
        }
        Ok(ident.ident.clone())
    }

    /// Parse a native function call `name(args..)` into the function's path
    /// and its arguments, resolved with [`Self::parse_native_args`]. Any arguments
    /// whose names have not yet been used will generate a new hole.
    ///
    /// Returns the path corresponding the the function's name and a list of
    /// the arguments' uses.
    pub(super) fn parse_native_call(
        &mut self,
        call: &syn::ExprCall,
    ) -> syn::Result<(syn::Path, Vec<Use>)> {
        let syn::Expr::Path(func) = call.func.as_ref() else {
            return Err(syn::Error::new_spanned(
                &call.func,
                "function call must be a path",
            ));
        };
        if let Some(segment) = func.path.segments.iter().find(|s| !s.arguments.is_none()) {
            return Err(syn::Error::new_spanned(
                &segment.arguments,
                "native functions in `#[rewrite_rule]` functions cannot have generic arguments",
            ));
        }
        Ok((func.path.clone(), self.parse_native_args(&call.args)?))
    }

    /// Resolve a native call's arguments with [`Self::parse_use`], in order.
    pub(super) fn parse_native_args<'a>(
        &mut self,
        args: impl IntoIterator<Item = &'a syn::Expr>,
    ) -> syn::Result<Vec<Use>> {
        args.into_iter().map(|arg| self.parse_use(arg)).collect()
    }

    /// Resolve an argument, which must be a plain name, alongside its span
    /// so errors can point to the use. `_` is only allowed as
    /// an operand of `op![..]`, see [`Self::parse_operand`].
    pub(super) fn parse_use(&mut self, arg: &syn::Expr) -> syn::Result<Use> {
        if let syn::Expr::Infer(infer) = arg {
            return Err(syn::Error::new_spanned(
                infer,
                "`_` can only be an argument of `op![..]` in `#[rewrite_rule]` functions",
            ));
        }
        let syn::Expr::Path(path) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "only identifiers are allowed as arguments in `#[rewrite_rule]` functions",
            ));
        };
        let Some(ident) = path.path.get_ident() else {
            return Err(syn::Error::new_spanned(
                path,
                "only single-segment paths are allowed as arguments in `#[rewrite_rule]` functions",
            ));
        };
        Ok(Use {
            value: self.resolve_value(ident)?,
            span: arg.span(),
        })
    }

    /// Resolve an operand of `op![..]` like [`Self::parse_use`], except that
    /// `_` is a fresh anonymous hole. Only an op's operands can be one:
    /// matching binds a hole by reading the operand slot it sits in.
    pub(super) fn parse_operand(&mut self, arg: &syn::Expr) -> syn::Result<Use> {
        let syn::Expr::Infer(infer) = arg else {
            return self.parse_use(arg);
        };
        let span = infer.underscore_token.span;
        Ok(Use {
            value: self.add_hole(None, span),
            span,
        })
    }

    /// Parse `op![Dialect::op(args..)]`, the initializer of a `let` binding an
    /// op's results.
    pub(super) fn parse_op(&mut self, mac: &syn::Macro) -> syn::Result<StmtKind> {
        if !mac.path.is_ident("op") {
            return Err(syn::Error::new_spanned(
                mac,
                "only `op!` macro invocations are allowed as initializers in `#[rewrite_rule]` functions",
            ));
        }
        if !matches!(mac.delimiter, syn::MacroDelimiter::Bracket(_)) {
            return Err(syn::Error::new_spanned(
                mac,
                "expected square brackets in `op![..]`",
            ));
        }

        let call: syn::ExprCall = mac.parse_body()?;
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return Err(syn::Error::new_spanned(
                &call.func,
                "function call must be a path",
            ));
        };
        let op = Self::parse_op_path(path)?;

        let args = call
            .args
            .iter()
            .map(|arg| self.parse_operand(arg))
            .collect::<syn::Result<_>>()?;
        Ok(StmtKind::Op { op, args })
    }

    /// Check that `path` names an op as `Dialect::op`, with `op` the snake-case
    /// name of the dialect's variant or struct: the code generated for the
    /// rule calls the dialect's `new_op` constructor.
    fn parse_op_path(path: &syn::ExprPath) -> syn::Result<syn::Path> {
        let segments = &path.path.segments;
        let (Some(op), None) = (segments.last(), &path.qself) else {
            return Err(syn::Error::new_spanned(path, OP_PATH));
        };
        if segments.len() < 2 {
            return Err(syn::Error::new_spanned(path, OP_PATH));
        }
        if !op.arguments.is_none() {
            return Err(syn::Error::new_spanned(
                &op.arguments,
                "the op in `op![..]` cannot have generic arguments",
            ));
        }
        let name = op.ident.unraw().to_string();
        let snake = to_snake_case(&name);
        if name != snake {
            return Err(syn::Error::new_spanned(
                &op.ident,
                format!(
                    "ops in `op![..]` are written in snake case: use `{snake}` instead of `{name}`"
                ),
            ));
        }
        Ok(path.path.clone())
    }

    /// Parse the function's last statement: `rewrite!(root, { .. })`, or a
    /// lone `replace!` or `erase!`, without a semicolon.
    pub(super) fn parse_rewrite_statement(&mut self, stmt: &Stmt) -> syn::Result<StmtId> {
        // `syn` parses a final macro call without a semicolon as an
        // expression statement.
        let Stmt::Expr(syn::Expr::Macro(syn::ExprMacro { mac, .. }), None) = stmt else {
            return Err(syn::Error::new_spanned(stmt, MISSING_REWRITE));
        };

        if mac.path.is_ident("rewrite") {
            self.parse_rewrite(mac)
        } else if mac.path.is_ident("replace") || mac.path.is_ident("erase") {
            // A lone action is shorthand for `rewrite!(matched, { action; })`.
            let (action, root) = self.parse_rewrite_action(mac)?;
            self.add_statement(
                mac.span(),
                StmtKind::Rewrite {
                    root,
                    body: vec![action],
                    walk_result: None,
                },
                vec![],
            )
        } else {
            Err(syn::Error::new_spanned(stmt, MISSING_REWRITE))
        }
    }

    /// Parse `rewrite!(root, { .. })`. The body's statements are added before
    /// the `rewrite!` itself.
    pub(super) fn parse_rewrite(&mut self, mac: &syn::Macro) -> syn::Result<StmtId> {
        let args = Self::macro_args(mac)?;
        let [root_expr, body] = args.as_slice() else {
            return Err(syn::Error::new_spanned(
                mac,
                "expected `rewrite!(root, { .. })`",
            ));
        };
        // The root is parsed before the body, so it cannot name an op the
        // body builds.
        let root = self.resolve_op(
            root_expr,
            "the root of the rewrite must be an `op![..]` matched before it, or one of its results",
        )?;

        let syn::Expr::Block(body) = body else {
            return Err(syn::Error::new_spanned(
                body,
                "the body of `rewrite!` must be a block",
            ));
        };
        // A tail expression is the walk result; everything before it, or the
        // whole body if it has none, are the actions.
        let (tail, actions) = match body.block.stmts.split_last() {
            Some((last, init)) if Self::is_walk_result_tail(last) => (Some(last), init),
            _ => (None, body.block.stmts.as_slice()),
        };
        let mut stmts: Vec<StmtId> = actions
            .iter()
            .map(|stmt| self.parse_replacement(stmt))
            .collect::<syn::Result<_>>()?;

        // A body that only builds ops creates values that are never used,
        // and one that only has a walk result leaves the IR unchanged.
        let acts = stmts.iter().any(|&stmt| {
            matches!(
                self.model().statement(stmt).kind,
                StmtKind::Erase { .. } | StmtKind::Replace { .. } | StmtKind::NativeCompute { .. }
            )
        });
        if !acts {
            return Err(syn::Error::new_spanned(
                body,
                "the body of `rewrite!` must contain an `erase!`, a `replace!` or a native call",
            ));
        }

        let walk_result = match tail {
            Some(tail) => {
                let (compute, walk_result) = self.parse_walk_result(tail)?;
                stmts.extend(compute);
                Some(walk_result)
            }
            None => None,
        };
        self.add_statement(
            mac.span(),
            StmtKind::Rewrite {
                root,
                body: stmts,
                walk_result,
            },
            vec![],
        )
    }

    /// Whether `stmt`, the last statement of a `rewrite!` body, is its walk
    /// result: an expression without a semicolon. `erase!` and `replace!`
    /// are not walk results: they must end in a semicolon, even when last.
    fn is_walk_result_tail(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Macro(syn::StmtMacro {
                mac,
                semi_token: None,
                ..
            })
            | Stmt::Expr(syn::Expr::Macro(syn::ExprMacro { mac, .. }), None) => {
                WALK_MACROS.iter().any(|name| mac.path.is_ident(name))
            }
            Stmt::Expr(_, None) => true,
            _ => false,
        }
    }

    /// Parse the walk result `tail`: `advance!()`, `skip!()`, `interrupt!()`,
    /// a native call, or the name of a value a native function in the
    /// `rewrite!` body computed. Returns the native call's statement, if it
    /// is one, and the [`StmtKind::WalkResult`].
    fn parse_walk_result(&mut self, tail: &Stmt) -> syn::Result<(Option<StmtId>, StmtId)> {
        let span = tail.span();
        let (compute, walk_result) = match tail {
            Stmt::Macro(syn::StmtMacro { mac, .. })
            | Stmt::Expr(syn::Expr::Macro(syn::ExprMacro { mac, .. }), _) => {
                let name = mac.path.get_ident().map(ToString::to_string);
                if !mac.tokens.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &mac.tokens,
                        format!("`{}!` takes no arguments", name.unwrap_or_default()),
                    ));
                }
                let walk_result = match name.as_deref() {
                    Some("advance") => WalkResult::Advance,
                    Some("skip") => WalkResult::Skip,
                    Some("interrupt") => WalkResult::Interrupt,
                    _ => unreachable!("`is_walk_result_tail` only accepts the walk macros"),
                };
                (None, walk_result)
            }
            Stmt::Expr(syn::Expr::Call(call), None) => {
                let (func, args) = self.parse_native_call(call)?;
                let compute = self.add_statement(
                    span,
                    StmtKind::NativeCompute { func, args },
                    vec![(None, span)],
                )?;
                let value = self.model().statement(compute).results[0];
                (Some(compute), WalkResult::Value(Use { value, span }))
            }
            Stmt::Expr(expr @ syn::Expr::Path(_), None) => {
                let value = self.parse_use(expr)?;
                if !self.model().is_rewrite_native_result(value.value) {
                    return Err(syn::Error::new_spanned(
                        expr,
                        "the walk result must be computed by a native function in the `rewrite!` body",
                    ));
                }
                (None, WalkResult::Value(value))
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    tail,
                    "the last expression of the body of `rewrite!` must be `advance!()`, `skip!()`, `interrupt!()`, a native call or the name of a native call's result",
                ));
            }
        };
        let walk_result = self.add_statement(span, StmtKind::WalkResult(walk_result), vec![])?;
        Ok((compute, walk_result))
    }

    /// Parse a statement of a `rewrite!` body: a `let` binding a native call
    /// or an `op![..]`, `erase!(..);` or `replace!(..);`.
    pub(super) fn parse_replacement(&mut self, stmt: &Stmt) -> syn::Result<StmtId> {
        match stmt {
            Stmt::Local(local) => {
                // Reject s @ (y_1, ..., y_n)
                // An op built in the rewrite body is never the root, replaced,
                // or erased, so a name for it could never be used.
                if let syn::Pat::Ident(syn::PatIdent {
                    subpat: Some((at, _)),
                    ..
                }) = &local.pat
                {
                    return Err(syn::Error::new_spanned(
                        at,
                        "`@` cannot name an statement built in the body of `rewrite!`",
                    ));
                }
                let stmt = self.parse_local(local)?;
                if let StmtKind::Op { args, .. } = &self.model().statement(stmt).kind
                    && args
                        .iter()
                        .any(|arg| self.model().is_matched_native_result(arg.value))
                {
                    return Err(syn::Error::new_spanned(
                        local,
                        "an `op![..]` cannot take the result of a native function called before the rewrite: only native functions in the `rewrite!` body can build IR",
                    ));
                }
                Ok(stmt)
            }
            Stmt::Macro(syn::StmtMacro {
                mac,
                semi_token: Some(_),
                ..
            }) => self.parse_rewrite_action(mac).map(|(action, _)| action),
            _ => Err(syn::Error::new_spanned(
                stmt,
                "only `let` bindings, `erase!(..);` and `replace!(..);` are allowed in the body of `rewrite!`",
            )),
        }
    }

    /// Parse `erase!(matched)` or `replace!(matched, replacement)`, returning
    /// the statement and the matched op it acts on.
    pub(super) fn parse_rewrite_action(
        &mut self,
        mac: &syn::Macro,
    ) -> syn::Result<(StmtId, StmtId)> {
        let (kind, op) = if mac.path.is_ident("erase") {
            let args = Self::macro_args(mac)?;
            let [matched_expr] = args.as_slice() else {
                return Err(syn::Error::new_spanned(mac, "expected `erase!(matched)`"));
            };
            let op = self.resolve_op(
                matched_expr,
                "`erase!` can only act on an `op![..]` before the rewrite, or one of its results",
            )?;
            (StmtKind::Erase { stmt: op }, op)
        } else if mac.path.is_ident("replace") {
            let args = Self::macro_args(mac)?;
            let [matched_expr, replacement_expr] = args.as_slice() else {
                return Err(syn::Error::new_spanned(
                    mac,
                    "expected `replace!(matched, replacement)`",
                ));
            };
            let matched = self.parse_use(matched_expr)?;
            if !self.model().is_matched_op_result(matched.value) {
                return Err(syn::Error::new_spanned(
                    matched_expr,
                    "`replace!` can only act on the result of an `op![..]` before the rewrite",
                ));
            }
            let replacement = self.parse_use(replacement_expr)?;
            if self.model().is_matched_native_result(replacement.value) {
                return Err(syn::Error::new_spanned(
                    replacement_expr,
                    "cannot `replace!` with the result of a native function called before the rewrite: only native functions in the `rewrite!` body can build a replacement",
                ));
            }
            if replacement.value == matched.value {
                return Err(syn::Error::new_spanned(
                    replacement_expr,
                    "`replace!` cannot redirect a value to itself",
                ));
            }

            (
                StmtKind::Replace {
                    matched,
                    replacement,
                },
                self.model().value(matched.value).stmt,
            )
        } else if mac.path.is_ident("rewrite") {
            return Err(syn::Error::new_spanned(
                mac,
                "`rewrite!` macros cannot be nested",
            ));
        } else if let Some(name) = WALK_MACROS.iter().find(|name| mac.path.is_ident(name)) {
            return Err(syn::Error::new_spanned(
                mac,
                format!(
                    "`{name}!()` can only be the last expression of the body of `rewrite!`, without a semicolon"
                ),
            ));
        } else {
            return Err(syn::Error::new_spanned(
                mac,
                "expected `erase!` or `replace!` in the body of `rewrite!`",
            ));
        };
        Ok((self.add_statement(mac.span(), kind, vec![])?, op))
    }

    /// A macro's comma-separated arguments.
    fn macro_args(mac: &syn::Macro) -> syn::Result<Vec<syn::Expr>> {
        Ok(mac
            .parse_body_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)?
            .into_iter()
            .collect())
    }
}
