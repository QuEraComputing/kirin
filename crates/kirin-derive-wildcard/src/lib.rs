extern crate proc_macro;

mod codegen;
mod model;
mod parse;
mod rewrite_rule;

use proc_macro::TokenStream;

/// Turn a Rust function whose body describes a rewrite rule into a function
/// `fn rule(stage: &mut impl AsBuildStage<PatternLanguage<L>>) -> Statement`
/// that builds the pattern in `stage` and returns its `wildcard.pattern`
/// statement.
#[proc_macro_attribute]
pub fn rewrite_rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    match rewrite_rule::expand(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
