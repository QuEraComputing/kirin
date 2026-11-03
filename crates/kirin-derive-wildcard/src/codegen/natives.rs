use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

use super::RuleCodegen;
use crate::model::{StmtId, StmtKind, Unpack};

impl RuleCodegen {
    /// The native functions the rule calls, attached to its `PatternRule`:
    /// a `.check_at(..)` for each native call in the match and an
    /// `.action_at(..)` for each one in the `rewrite!` body.
    ///
    /// Each attaches a closure that reads the call's arguments, calls the
    /// function and wraps its results. Every call gets its own closure, so
    /// each is compiled against the function, and one function may be called
    /// in several ways, e.g. with and without `?`.
    pub(super) fn natives(&self) -> TokenStream {
        let (_, rewrite_body) = self.model.rewrite();
        let checks = self.native_calls(self.model.body()).map(|s| self.check(s));
        let actions = self.native_calls(rewrite_body).map(|s| self.action(s));
        quote! { #(#checks)* #(#actions)* }
    }

    /// The native calls among `stmts`.
    fn native_calls<'a>(&'a self, stmts: &'a [StmtId]) -> impl Iterator<Item = StmtId> + 'a {
        stmts.iter().copied().filter(|&stmt| {
            matches!(
                self.model.statement(stmt).kind,
                StmtKind::NativeCompute { .. }
            )
        })
    }

    /// `.check_at(statement, ..)` for the native call `stmt` in the match. The function
    /// takes the `&StageInfo<L>` being matched first. A `?` call returns a
    /// `CheckOutcome`: an `Option`, whose `None` fails the match, or a
    /// `Result`, whose `Err` fails the run. A constraint's returns a `bool`,
    /// or a `CheckOutcome` of one if it ends in `?`.
    fn check(&self, stmt: StmtId) -> TokenStream {
        let krate = &self.args.crate_path;
        let span = self.native_span(stmt);
        let stage = syn::Ident::new("native_stage", Span::mixed_site());
        let (call, fallible) = self.native_call(stmt, &stage);
        let statement = self.statement_ident(stmt);
        let result = Self::native_result();
        // Checked at the call, so a function returning something else is
        // reported there.
        let mut checked = result.clone();
        checked.set_span(span);
        let into_check_result: TokenStream = quote!(#krate::CheckOutcome::into_check_result)
            .into_iter()
            .map(|mut token| {
                token.set_span(span);
                token
            })
            .collect();

        let unwrap = fallible.then(|| {
            quote_spanned! {span=>
                let ::core::option::Option::Some(#result) =
                    #into_check_result(#checked)?
                else {
                    return ::core::result::Result::Ok(::core::option::Option::None);
                };
            }
        });
        let constraint = self
            .model
            .is_constraint_call(stmt)
            .then(|| quote_spanned! {span=> let #result: bool = #checked; });
        let values = self.native_values(stmt);
        let args = Self::native_args();
        quote! {
            .check_at(#statement, |#stage, #args: &[#krate::NativeValue]| {
                let #result = #call;
                #unwrap
                #constraint
                ::core::result::Result::Ok(::core::option::Option::Some(#values))
            })
        }
    }

    /// `.action_at(statement, ..)` for the native call `stmt` in the `rewrite!` body.
    /// The function takes the pass's `&mut Rewriter<L>` first. A `?` call
    /// returns a `Result`, whose `Err` fails the rewrite.
    fn action(&self, stmt: StmtId) -> TokenStream {
        let krate = &self.args.crate_path;
        let span = self.native_span(stmt);
        let rewriter = syn::Ident::new("native_rewriter", Span::mixed_site());
        let (call, fallible) = self.native_call(stmt, &rewriter);
        let statement = self.statement_ident(stmt);
        let result = Self::native_result();

        let unwrap = fallible.then(|| quote_spanned! {span=> let #result = #result?; });
        let values = self.native_values(stmt);
        let args = Self::native_args();
        quote! {
            .action_at(#statement, |#rewriter, #args: &[#krate::NativeValue]| {
                let #result = #call;
                #unwrap
                ::core::result::Result::Ok(#values)
            })
        }
    }

    /// The call `func(context, NativeValue::arg(args, 0)?, ..)` of the
    /// native call `stmt`, and whether it ends in `?`. Each argument is read
    /// as the type the function takes.
    fn native_call(&self, stmt: StmtId, context: &syn::Ident) -> (TokenStream, bool) {
        let StmtKind::NativeCompute {
            func,
            args,
            fallible,
            ..
        } = &self.model.statement(stmt).kind
        else {
            unreachable!("`native_calls` yields only native calls")
        };
        let krate = &self.args.crate_path;
        let span = self.native_span(stmt);
        let native_args = Self::native_args();
        let values = (0..args.len()).map(|position| {
            quote_spanned! {span=> #krate::NativeValue::arg(#native_args, #position)? }
        });
        // `#func` keeps its own spans, so it resolves where the rule is
        // written and a missing function is reported at it.
        let call = quote_spanned! {span=> #func(#context, #(#values),*) };
        (call, *fallible)
    }

    /// The function's result as the `Vec<NativeValue>` of the call's
    /// results, taken apart as the call's `let` pattern does: the whole
    /// value for `v`, or a tuple pattern binding each result, so `(a,)`
    /// binds the element of a 1-tuple and `()` binds nothing.
    fn native_values(&self, stmt: StmtId) -> TokenStream {
        let krate = &self.args.crate_path;
        let span = self.native_span(stmt);
        let result = Self::native_result();
        let StmtKind::NativeCompute { unpack, .. } = &self.model.statement(stmt).kind else {
            unreachable!("only native calls have native values")
        };
        if *unpack == Unpack::Whole {
            return quote! { ::std::vec![#krate::NativeValue::new(#result)] };
        }
        let mut parts = Vec::new();
        let pattern = Self::unpack_pattern(unpack, &mut parts);
        quote_spanned! {span=> {
            let #pattern = #result;
            ::std::vec![#(#krate::NativeValue::new(#parts)),*]
        }}
    }

    /// The Rust pattern taking a value apart as `unpack`, binding each leaf
    /// to a new local pushed onto `parts`.
    fn unpack_pattern(unpack: &Unpack, parts: &mut Vec<syn::Ident>) -> TokenStream {
        match unpack {
            Unpack::Whole => {
                let part =
                    format_ident!("native_result_{}", parts.len(), span = Span::mixed_site());
                parts.push(part.clone());
                quote!(#part)
            }
            // A trailing comma keeps a 1-tuple a tuple pattern.
            Unpack::Tuple(elems) if elems.len() == 1 => {
                let elem = Self::unpack_pattern(&elems[0], parts);
                quote!((#elem,))
            }
            Unpack::Tuple(elems) => {
                let elems: Vec<_> = elems
                    .iter()
                    .map(|e| Self::unpack_pattern(e, parts))
                    .collect();
                quote!((#(#elems),*))
            }
        }
    }

    /// Where errors about the native call `stmt` are reported: the function's
    /// name, rather than the `let` the statement starts with.
    fn native_span(&self, stmt: StmtId) -> Span {
        let StmtKind::NativeCompute { func, .. } = &self.model.statement(stmt).kind else {
            unreachable!("only native calls have a native span")
        };
        Span::mixed_site().located_at(func.span())
    }

    /// The local holding a native function's result.
    fn native_result() -> syn::Ident {
        syn::Ident::new("native_result", Span::mixed_site())
    }

    /// The closure parameter holding a native call's arguments.
    fn native_args() -> syn::Ident {
        syn::Ident::new("native_args", Span::mixed_site())
    }
}
