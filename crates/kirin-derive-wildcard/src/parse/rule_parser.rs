use std::collections::HashMap;

use super::names::Name;
use crate::model::{RuleModel, StmtId};

pub(super) const MISSING_REWRITE: &str = "a `#[rewrite_rule]` function must end with \
    `rewrite!(root, { .. })`, `replace!(matched, replacement)` or `erase!(matched)`";

/// Parses a `#[rewrite_rule]` function into a [`RuleModel`].
///
/// The function body is the match, a list of `let` bindings and constraint
/// calls, followed by the replacement: either a `rewrite!(root, { .. })`,
/// or a lone `replace!` or `erase!`.
pub(crate) struct RuleParser {
    pub(super) model: RuleModel,
    /// What each name refers to so far: its `let` binding, or the hole
    /// created at its first use.
    pub(super) names: HashMap<String, Name>,
}

impl RuleParser {
    /// Parse `item` into a [`RuleModel`].
    pub(crate) fn parse(item: &syn::ItemFn) -> syn::Result<RuleModel> {
        Self::check_signature(&item.sig)?;
        let mut parser = Self::new(item.sig.ident.clone());

        // Separate the last statement in the function body to be the replacement
        let Some((last, rest)) = item.block.stmts.split_last() else {
            return Err(syn::Error::new_spanned(&item.block, MISSING_REWRITE));
        };

        // Parse the statements in the matching section of the pattern
        for stmt in rest {
            for id in parser.parse_statement(stmt)? {
                parser.push_body(id);
            }
        }

        // Parse the statements in the rewrite section of the pattern
        let id = parser.parse_rewrite_statement(last)?;
        parser.push_body(id);

        parser.model.verify()?;

        Ok(parser.model)
    }

    pub(super) fn new(name: syn::Ident) -> Self {
        Self {
            model: RuleModel::new(name),
            names: HashMap::new(),
        }
    }

    /// Append statemnt `id` to the rule's body: the statements of the match,
    /// and finally the rewrite, excluding its inner statements.
    pub(super) fn push_body(&mut self, id: StmtId) {
        self.model.push_body(id);
    }

    /// The model built so far.
    pub(super) fn model(&self) -> &RuleModel {
        &self.model
    }

    /// Generic parameters and a where clause are kept on the generated
    /// function, so a rule can be built for any language its bounds admit.
    pub(super) fn check_signature(sig: &syn::Signature) -> syn::Result<()> {
        if sig.asyncness.is_some() {
            return Err(syn::Error::new_spanned(
                sig.asyncness,
                "`#[rewrite_rule]` functions cannot be async",
            ));
        }
        if sig.constness.is_some() {
            return Err(syn::Error::new_spanned(
                sig.constness,
                "`#[rewrite_rule]` functions cannot be const",
            ));
        }
        if sig.unsafety.is_some() {
            return Err(syn::Error::new_spanned(
                sig.unsafety,
                "`#[rewrite_rule]` functions cannot be unsafe",
            ));
        }
        if sig.abi.is_some() {
            return Err(syn::Error::new_spanned(
                &sig.abi,
                "`#[rewrite_rule]` functions cannot have an ABI",
            ));
        }
        if sig.variadic.is_some() {
            return Err(syn::Error::new_spanned(
                &sig.variadic,
                "`#[rewrite_rule]` functions cannot be variadic",
            ));
        }
        // The generated function has a fixed signature, returning the pattern.
        if let syn::ReturnType::Type(..) = sig.output {
            return Err(syn::Error::new_spanned(
                &sig.output,
                "`#[rewrite_rule]` functions cannot have a return type",
            ));
        }
        Ok(())
    }
}
