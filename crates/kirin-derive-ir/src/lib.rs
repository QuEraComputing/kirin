extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod generate;

use generate::*;

#[proc_macro_derive(Dialect, attributes(kirin, wraps))]
pub fn derive_statement(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match generate_dialect(&ast) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(e) => TokenStream::from(e.write_errors()),
    }
}

fn do_derive_field_iter(input: TokenStream, config: LocalFieldIterConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match generate_field_iter(&ast, config) {
        Ok(t) => TokenStream::from(t),
        Err(e) => TokenStream::from(e.write_errors()),
    }
}

fn do_derive_property(input: TokenStream, config: LocalPropertyConfig) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match generate_property(&ast, config) {
        Ok(t) => TokenStream::from(t),
        Err(e) => TokenStream::from(e.write_errors()),
    }
}

macro_rules! derive_field_iter_macro {
    ($fn_name:ident, $trait_name:ident, $config:ident) => {
        #[proc_macro_derive($trait_name, attributes(kirin, wraps))]
        pub fn $fn_name(input: TokenStream) -> TokenStream {
            do_derive_field_iter(input, $config)
        }
    };
}

macro_rules! derive_property_macro {
    ($fn_name:ident, $trait_name:ident, $config:ident) => {
        #[proc_macro_derive($trait_name, attributes(kirin, wraps))]
        pub fn $fn_name(input: TokenStream) -> TokenStream {
            do_derive_property(input, $config)
        }
    };
}

derive_field_iter_macro!(derive_has_arguments, HasArguments, HAS_ARGUMENTS);
derive_field_iter_macro!(derive_has_arguments_mut, HasArgumentsMut, HAS_ARGUMENTS_MUT);
derive_field_iter_macro!(derive_has_results, HasResults, HAS_RESULTS);
derive_field_iter_macro!(derive_has_results_mut, HasResultsMut, HAS_RESULTS_MUT);
derive_field_iter_macro!(derive_has_blocks, HasBlocks, HAS_BLOCKS);
derive_field_iter_macro!(derive_has_blocks_mut, HasBlocksMut, HAS_BLOCKS_MUT);
derive_field_iter_macro!(derive_has_successors, HasSuccessors, HAS_SUCCESSORS);
derive_field_iter_macro!(
    derive_has_successors_mut,
    HasSuccessorsMut,
    HAS_SUCCESSORS_MUT
);
derive_field_iter_macro!(derive_has_regions, HasRegions, HAS_REGIONS);
derive_field_iter_macro!(derive_has_regions_mut, HasRegionsMut, HAS_REGIONS_MUT);

derive_property_macro!(derive_is_terminator, IsTerminator, IS_TERMINATOR);
derive_property_macro!(derive_is_constant, IsConstant, IS_CONSTANT);
derive_property_macro!(derive_is_pure, IsPure, IS_PURE);
derive_property_macro!(derive_is_speculatable, IsSpeculatable, IS_SPECULATABLE);

#[proc_macro_derive(StageMeta, attributes(stage))]
pub fn derive_stage_meta(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match kirin_derive_toolkit::stage_info::generate(&ast) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}

/// Derive macro that generates a monomorphic [`ParseDispatch`] implementation
/// for stage enums. Uses the same `#[stage(...)]` attributes as `StageMeta`.
///
/// # Optional attributes
///
/// - `#[stage(chumsky_crate = "path")]` — override the path to the kirin-chumsky
///   crate (default: `::kirin::parsers`).
///
/// # Example
///
/// ```ignore
/// #[derive(StageMeta, ParseDispatch)]
/// #[stage(crate = "kirin_ir", chumsky_crate = "kirin_chumsky")]
/// enum MixedStage {
///     #[stage(name = "A")]
///     StageA(StageInfo<FunctionBody>),
///     #[stage(name = "B")]
///     StageB(StageInfo<LowerBody>),
/// }
/// ```
#[proc_macro_derive(ParseDispatch, attributes(stage))]
pub fn derive_parse_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match kirin_derive_toolkit::parse_dispatch::generate(&ast) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}
