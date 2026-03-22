extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod generate;

/// Derives `RenderDispatch` for stage enums.
///
/// Generates a match-arm dispatch implementation that delegates
/// `render_staged_function` to each variant's inner `StageInfo<L>` type.
/// This is the stage-level counterpart of `PrettyPrint` for individual
/// dialect types.
///
/// # When to use
///
/// Add `#[derive(RenderDispatch)]` alongside `#[derive(StageMeta)]` on
/// stage enums that participate in pipeline pretty-printing. Each inner
/// dialect type (the `L` in `StageInfo<L>`) must implement `PrettyPrint`.
///
/// # Attributes
///
/// - `#[stage(crate = "...")]` (required) -- path to the IR crate,
///   forwarded from `StageMeta`.
/// - `#[pretty(crate = ...)]` (optional) -- path to the prettyless crate.
///   Defaults to `::kirin::pretty`.
///
/// # Example
///
/// ```ignore
/// #[derive(StageMeta, RenderDispatch)]
/// #[stage(crate = "kirin_ir")]
/// enum MyStage {
///     #[stage(name = "source")]
///     Source(StageInfo<HighLevel>),
///     #[stage(name = "lowered")]
///     Lowered(StageInfo<LowLevel>),
/// }
/// ```
#[proc_macro_derive(RenderDispatch, attributes(stage, pretty))]
pub fn derive_render_stage(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match generate::generate(&ast) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}
