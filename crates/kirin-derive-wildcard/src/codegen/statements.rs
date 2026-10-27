use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{ext::IdentExt, spanned::Spanned};

use super::RuleCodegen;
use crate::model::{StmtId, StmtKind, Use, ValueId};

impl RuleCodegen {
    /// The code building statement `id`, by its kind.
    pub(super) fn statement(&self, id: StmtId) -> TokenStream {
        match &self.model.statement(id).kind {
            StmtKind::Hole => self.hole(id),
            StmtKind::Op { op, args } => self.op(id, op, args),
            StmtKind::NativeCompute { func, args } => self.native_compute(id, func, args),
            StmtKind::Constraint { success } => self.constraint(id, *success),
            StmtKind::Rewrite { root, body } => self.rewrite(id, *root, body),
            StmtKind::Replace {
                matched,
                replacement,
            } => self.replace(id, matched.value, replacement.value),
            StmtKind::Erase { stmt: op } => self.erase(id, *op),
        }
    }

    /// Reserve the id of `stmt` and create its results in order.
    /// The `stmt` needs to be reserved ahead of the results since these
    /// must point at the statement defining it before that statement exists.
    ///
    /// Returns the code generating the `stmt` id and the `ResultValue`s,
    /// alongside the results' idents in order.
    pub(super) fn results(&self, stmt: StmtId) -> (TokenStream, Vec<syn::Ident>) {
        let ir = self.ir_path();
        let ty = self.type_alias();
        let stage = self.stage_ident();
        let statement = self.statement_ident(stmt);
        let values = &self.model.statement(stmt).results;

        let idents: Vec<_> = values.iter().map(|&id| self.value_ident(id)).collect();
        let declarations = values
            .iter()
            .zip(&idents)
            .enumerate()
            .map(|(index, (&id, value))| {
                // Give the SSA value the name from the rule's binding, unless it is `_`.
                let name = self.model.value(id).name.as_ref().map(|name| {
                    let name = name.unraw().to_string();
                    quote! { .name(#name) }
                });
                quote! {
                    let #value: #ir::ResultValue = #stage
                        .ssa()
                        #name
                        .ty(<#ty as #ir::Placeholder>::placeholder())
                        .kind(#ir::BuilderSSAKind::Result(#statement, #index))
                        .new()
                        .into();
                }
            });
        let code = quote! {
            let #statement = #stage.statement_arena().next_id();
            #(#declarations)*
        };
        (code, idents)
    }

    /// Cast an operand `value` as an `SSAValue` which will have been bound
    /// to some `ResultValue` above.
    pub(super) fn operand(&self, value: Use) -> TokenStream {
        let ir = self.ir_path();
        let span = Span::mixed_site().located_at(value.span);

        // Only the location moves: the local keeps its mixed-site hygiene,
        // so it still resolves to the value's declaration.
        let mut local = self.value_ident(value.value);
        local.set_span(span);
        quote_spanned! {span=> <#ir::SSAValue>::from(#local) }
    }

    /// Generate a `wildcard.hole` for the hole statement `stmt`, whose value
    /// is named after the rule's name for it unless it is `_`.
    pub(super) fn hole(&self, stmt: StmtId) -> TokenStream {
        let krate = &self.args.crate_path;
        let ty = self.type_alias();
        let stage = self.stage_ident();

        let (declare_results, result_values) = self.results(stmt);
        quote! {
            #declare_results
            #stage
                .statement()
                .definition(#krate::Hole::<#ty>::new_hole(#(#result_values),*))
                .new();
        }
    }

    /// `let r = op![Arith::mul(x, c)];`: an op of the language, as
    /// `PatternLanguage::Op(..)`, built by the dialect's generated constructor
    /// `Arith::new_mul(x, c, r)` and converted into the pattern's language
    /// via its `From` impl.
    ///
    /// Operands are passed as `SSAValue` and results as `ResultValue`, so an
    /// op declaring its fields in another order fails to compile.
    pub(super) fn op(&self, stmt: StmtId, op: &syn::Path, args: &[Use]) -> TokenStream {
        let krate = &self.args.crate_path;
        let language = &self.args.language;
        let stage = self.stage_ident();

        // `Arith::add` becomes `Arith::new_add`, located at `add` so a
        // missing op is reported there.
        let mut constructor = op.clone();
        let name = &mut constructor
            .segments
            .last_mut()
            .expect("the parser checks that an op path names its dialect")
            .ident;
        let span = Span::mixed_site().located_at(name.span());
        *name = format_ident!("new_{}", name.unraw(), span = span);

        // The call to `from` for converting to the pattern's language is
        // spanned at the op to report there when the language does not
        // contain that op's dialect.
        let into_language = quote_spanned! {op.span()=> ::core::convert::From::from };

        let (declare_results, result_values) = self.results(stmt);
        let operands = args.iter().map(|&arg| self.operand(arg));
        quote! {
            #declare_results
            #stage
                .statement()
                .definition(#krate::PatternLanguage::<#language>::Op(#into_language(
                    #constructor(#(#operands,)* #(#result_values),*)
                )))
                .new();
        }
    }

    /// `let log_val = math::log2(val);`: a `wildcard.native_call` named
    /// `"math::log2"` computing the results of `stmt`
    pub(super) fn native_compute(
        &self,
        stmt: StmtId,
        func: &syn::Path,
        args: &[Use],
    ) -> TokenStream {
        let name = Self::native_name(func);
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();

        // `#func` keeps its own spans, so it resolves where the rule is
        // written and a missing function is reported at it.
        let span = Span::mixed_site().located_at(self.model.statement(stmt).span);
        // Have rustc check the results of the function
        let check = quote_spanned! {span=> let _ = #func; };

        let (declare_results, result_values) = self.results(stmt);
        let operands = args.iter().map(|&arg| self.operand(arg));
        quote! {
            #check
            #declare_results
            #stage
                .statement()
                .definition(#krate::NativeCall::<#ty>::new_native_call(
                    ::std::string::String::from(#name),
                    ::std::vec![#(#operands),*],
                    ::std::vec![#(#result_values),*],
                ))
                .new();
        }
    }

    /// The name a `wildcard.native_call` stores for `func`. The string
    /// contains the whole path, `::`-separated, with any leading `::` and
    /// dropping raw prefixes. The parser rejects generic arguments, so the
    /// segments' names are the whole path.
    pub(super) fn native_name(func: &syn::Path) -> String {
        let mut name = String::new();
        if func.leading_colon.is_some() {
            name.push_str("::");
        }
        for (index, segment) in func.segments.iter().enumerate() {
            if index > 0 {
                name.push_str("::");
            }
            name.push_str(&segment.ident.unraw().to_string());
        }
        name
    }

    /// `wildcard.constraint`, requiring `success` to hold: the second statement
    /// of a constraint call like `is_power_of_two(val);`. The `NativeCompute`
    /// producing `success` comes just before it.
    pub(super) fn constraint(&self, stmt: StmtId, success: ValueId) -> TokenStream {
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();

        let success_id = self.value_ident(success);
        let stmt_id = self.statement_ident(stmt);

        quote! {
            let #stmt_id = #krate::Constraint::<#ty>::new(#stage, #success_id).id;
        }
    }

    /// `rewrite!(root, { .. })`: the `wildcard.rewrite` terminator for the
    /// pattern block. Establishes `root` for the rewrite rule driver. `root`
    /// is identified by the `Statement` built for it in the pattern body.
    pub(super) fn rewrite(&self, stmt: StmtId, root: StmtId, body: &[StmtId]) -> TokenStream {
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();

        let root_id = self.statement_ident(root);
        let stmt_id = self.statement_ident(stmt);

        let span = Span::mixed_site().located_at(self.model.statement(stmt).span);
        let body_id = syn::Ident::new("rewrite_body", span);

        let body_quotes = body.iter().map(|&s| self.statement(s));
        let body_ids = body.iter().map(|&s| self.statement_ident(s));
        let block_name = format!("{}_rw", self.model.name());

        quote! {
            #(#body_quotes)*
            let #body_id = #stage
                .block()
                .name(#block_name)
                #(.stmt(#body_ids))*
                .new();
            let #stmt_id =
                #krate::Rewrite::<#ty>::new(#stage, #root_id, #body_id).id;
        }
    }

    /// `replace!(matched, replacement);`: a `wildcard.replace`.
    pub(super) fn replace(
        &self,
        stmt: StmtId,
        matched: ValueId,
        replacement: ValueId,
    ) -> TokenStream {
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();

        let matched_id = self.value_ident(matched);
        let replacement_id = self.value_ident(replacement);
        let stmt_id = self.statement_ident(stmt);

        quote! {
            let #stmt_id =
                #krate::Replace::<#ty>::new(#stage, #matched_id, #replacement_id).id;
        }
    }

    /// `erase!(matched);`: a `wildcard.erase` of the matched op `op` which is
    /// identified by the `Statement` built for it in the pattern body.
    pub(super) fn erase(&self, stmt: StmtId, op: StmtId) -> TokenStream {
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();

        let op_id = self.statement_ident(op);
        let stmt_id = self.statement_ident(stmt);

        quote! {
            let #stmt_id = #krate::Erase::<#ty>::new(#stage, #op_id).id;
        }
    }
}
