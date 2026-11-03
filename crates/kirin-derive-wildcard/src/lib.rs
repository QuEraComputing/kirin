extern crate proc_macro;

mod codegen;
mod model;
mod parse;
mod rewrite_rule;

use proc_macro::TokenStream;

/// Turn a Rust function whose body describes a rewrite rule into a function
/// `fn rule(stage: &mut impl AsBuildStage<PatternLanguage<L>>) -> PatternRule<L>`
/// that builds the pattern in `stage` and returns it with the native
/// functions it calls.
///
/// A native call is an ordinary Rust function, called with one more first
/// argument: `&StageInfo<L>` in the match, `&mut Rewriter<L>` in the
/// `rewrite!` body. Its other arguments are read as the types it takes. How
/// the rule calls it says what it returns:
///
/// | Call | Returns |
/// |------|---------|
/// | `f(x);` in the match (a constraint) | `bool`; `false` fails the match |
/// | `f(x)?;` in the match | `Option<bool>` or `Result<bool, E>`, unwrapped as for `let v = f(x)?;` before the `bool` is tested |
/// | `let v = f(x);` | the value |
/// | `let v = f(x)?;` in the match | `Option<_>`, whose `None` fails the match, or `Result<_, E>`, whose `Err` fails the run with the error (`E: Error + Send + Sync + 'static`) |
/// | `let v = f(x)?;` in the `rewrite!` body | `Result<_, RewriteError>`; `Err` fails the rewrite |
/// | `let (a, b) = f(x);` | a tuple, one value per name |
/// | `f(x);` in the `rewrite!` body | `()`, or a `Result` with `?` |
///
/// Each call is compiled on its own, so one function may be called in
/// several of these ways, e.g. both `f(x)?` and `f(x)`, or as a constraint
/// and for its `bool`.
///
/// The function may be generic, with its `language` parameter assigned to one
/// of the generic parameters. This allows the rule to apply to any language
/// that implement the `where` bounds the rule describes. Normally, these are
/// the dialects the rule should apply to  e.g. `L: From<Arith<L::Type>>`.
#[proc_macro_attribute]
pub fn rewrite_rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    match rewrite_rule::expand(attr.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
