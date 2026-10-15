use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::ext::IdentExt;

use crate::model::{RuleModel, StmtId, ValueId};
use crate::rewrite_rule::RuleArgs;

/// Generates the code a `#[rewrite_rule]` function expands into.
///
/// The expansion replaces the function with one of the same name and
/// visibility that builds the rule as pattern IR in the stage it is given.
/// Building into the caller's stage lets several rules share one stage,
/// which the caller finalizes at the end.
pub(crate) struct RuleCodegen {
    pub(super) model: RuleModel,
    /// The function the attribute is on, for its attributes and visibility.
    pub(super) item: syn::ItemFn,
    pub(super) args: RuleArgs,
}

impl RuleCodegen {
    /// Generate the expansion of `item`, which was parsed into `model`, with
    /// the attribute's `args`.
    pub(crate) fn generate(model: RuleModel, item: syn::ItemFn, args: RuleArgs) -> TokenStream {
        let codegen = Self::new(model, item, args);
        let signature = codegen.signature();
        let pattern = codegen.pattern();
        let language = &codegen.args.language;
        let ir = codegen.ir_path();
        let ty = codegen.type_alias();
        let stage = codegen.stage_ident();
        quote! {
            #signature {
                type #ty = <#language as #ir::Dialect>::Type;
                let #stage = #ir::AsBuildStage::as_build_stage(#stage);
                #pattern
            }
        }
    }

    pub(super) fn new(model: RuleModel, item: syn::ItemFn, args: RuleArgs) -> Self {
        Self { model, item, args }
    }

    /// The generated function's signature, keeping the attributes,
    /// visibility and name of the original. It takes the stage to build the
    /// pattern in and returns the `wildcard.pattern` statement.
    pub(super) fn signature(&self) -> TokenStream {
        let syn::ItemFn {
            attrs, vis, sig, ..
        } = &self.item;
        let name = &sig.ident;
        let krate = &self.args.crate_path;
        let language = &self.args.language;
        let ir = self.ir_path();
        let stage = self.stage_ident();
        quote! {
            #(#attrs)*
            #vis fn #name(
                #stage: &mut impl #ir::AsBuildStage<#krate::PatternLanguage<#language>>,
            ) -> #ir::Statement
        }
    }

    /// The path to `kirin_ir`, re-exported by `kirin_wildcard` so a crate
    /// using the macro does not need to depend on it.
    pub(super) fn ir_path(&self) -> TokenStream {
        let krate = &self.args.crate_path;
        quote!(#krate::__private::ir)
    }

    /// The alias the generated function declares for the IR type of the
    /// language, `<L as Dialect>::Type`, the `T` of the wildcard statements.
    pub(super) fn type_alias(&self) -> syn::Ident {
        syn::Ident::new("__Type", Span::mixed_site())
    }

    /// The generated function's parameter, and later a local variable holding
    /// the stage the pattern is built in, a `&mut BuilderStageInfo`.
    pub(super) fn stage_ident(&self) -> syn::Ident {
        syn::Ident::new("stage", Span::mixed_site())
    }

    /// The `wildcard.pattern` statement, named after the function. Consists
    /// of a block expression building the function's top-level statements
    /// with the `rewrite` as its terminator, and the pattern object itself. It
    /// evaluates to the pattern's `Statement`.
    pub(super) fn pattern(&self) -> TokenStream {
        let krate = &self.args.crate_path;
        let stage = self.stage_ident();
        let ty = self.type_alias();
        let rule_name = self.model.name().unraw().to_string();

        // Mixed-site, like the other locals, so they stay apart from the
        // rule's own names. There is only one pattern per rule.
        let span = Span::mixed_site().located_at(self.model.name().span());
        let body_id = syn::Ident::new("pattern_body", span);
        let name_id = syn::Ident::new("pattern_name", span);

        let body = self.model.body();
        let stmt_quotes = body.iter().map(|&s| self.statement(s));

        let (matched, rewrite) = self.model.split_rewrite();
        let stmt_ids = matched.iter().map(|&s| self.statement_ident(s));
        let rewrite_id = self.statement_ident(rewrite);
        let block_name = format!("{rule_name}_pat");

        quote! {{
            #(#stmt_quotes)*
            let #body_id = #stage
                .block()
                .name(#block_name)
                #(.stmt(#stmt_ids))*
                .terminator(#rewrite_id)
                .new();
            let #name_id = #stage
                .symbol_table_mut()
                .intern(::std::string::String::from(#rule_name));
            #krate::Pattern::<#ty>::new(#stage, #name_id, #body_id).id
        }}
    }

    /// Generates an ident for the local variables holding the `ResultValue`'s
    /// of the PatternLanguage statements we are building. The generated
    /// idents are `value_x` for a value named `x` in the user's rule, or
    /// `value_#` for an anonymous result.
    pub(super) fn value_ident(&self, id: ValueId) -> syn::Ident {
        let value = self.model.value(id);
        let span = Span::mixed_site().located_at(value.span);
        match &value.name {
            Some(name) => format_ident!("value_{}", name, span = span),
            None => format_ident!("value_{}", id.index(), span = span),
        }
    }

    /// The local holding the `Statement` built for `id`: `statement_3` for the
    /// model's statement 3.
    pub(super) fn statement_ident(&self, id: StmtId) -> syn::Ident {
        let span = Span::mixed_site().located_at(self.model.statement(id).span);
        format_ident!("statement_{}", id.index(), span = span)
    }
}
