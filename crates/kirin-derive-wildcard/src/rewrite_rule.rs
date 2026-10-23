use kirin_derive_toolkit::misc::debug_dump;
use kirin_derive_toolkit::prelude::darling::{FromMeta, ast::NestedMeta};
use proc_macro2::TokenStream;

use crate::{codegen::RuleCodegen, parse::RuleParser};

/// The arguments of `#[rewrite_rule(language = L, crate = path)]`.
#[derive(FromMeta)]
pub(crate) struct RuleArgs {
    /// The language `L` the pattern is written in, as `PatternLanguage<L>`.
    pub language: syn::Path,
    /// The path to the `kirin_wildcard` crate.
    #[darling(rename = "crate", default = || syn::parse_quote!(::kirin_wildcard))]
    pub crate_path: syn::Path,
}

/// Expand `#[rewrite_rule]` on a function into a `RewriteRule` constructor.
pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let args = RuleArgs::from_list(&NestedMeta::parse_meta_list(attr)?)?;
    let item: syn::ItemFn = syn::parse2(item)?;
    let model = RuleParser::parse(&item)?;
    let tokens = RuleCodegen::generate(model, item, args);
    debug_dump(&tokens);
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::quote;

    use super::expand;

    /// The expansion of a small valid rule with the attribute arguments
    /// `attr`, as a string.
    fn expand_rule(attr: TokenStream) -> syn::Result<String> {
        let item = quote! {
            fn rule() {
                let r = op![Arith::neg(x)];
                replace!(r, x)
            }
        };
        expand(attr, item).map(|tokens| tokens.to_string())
    }

    fn error(attr: TokenStream, item: TokenStream) -> String {
        expand(attr, item).unwrap_err().to_string()
    }

    #[test]
    fn expand_defaults_the_crate_path_to_kirin_wildcard() {
        let code = expand_rule(quote!(language = L)).unwrap();
        let pattern_language = quote!(::kirin_wildcard::PatternLanguage<L>).to_string();
        assert!(code.contains(&pattern_language), "{code}");
    }

    #[test]
    fn expand_uses_the_given_crate_path() {
        let code = expand_rule(quote!(language = L, crate = my_wildcard)).unwrap();
        let pattern_language = quote!(my_wildcard::PatternLanguage<L>).to_string();
        assert!(code.contains(&pattern_language), "{code}");
        assert!(!code.contains("kirin_wildcard"), "{code}");
    }

    #[test]
    fn expand_rejects_bad_arguments() {
        let cases: [(TokenStream, &str); 2] = [
            (quote!(), "Missing field `language`"),
            (
                quote!(language = L, color = red),
                "Unknown field: `color`. Available values: `crate`, `language`",
            ),
        ];
        for (attr, message) in cases {
            assert_eq!(expand_rule(attr).unwrap_err().to_string(), message);
        }
    }

    #[test]
    fn expand_rejects_an_item_that_is_not_a_function() {
        assert_eq!(
            error(
                quote!(language = L),
                quote!(
                    struct Rule;
                )
            ),
            "expected `fn`"
        );
    }

    #[test]
    fn expand_reports_an_invalid_rule() {
        let item = quote! {
            fn rule() {
                let r = op![Arith::neg(x)];
                erase!(x)
            }
        };
        assert_eq!(
            error(quote!(language = L), item),
            "`erase!` can only act on an `op![..]` before the rewrite, or one of its results"
        );
    }
}
