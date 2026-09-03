extern crate proc_macro;

mod frame_build;
mod function_entry;
mod interp_dispatch;
mod interpretable;
mod layout;

use proc_macro::TokenStream;
use syn::parse_macro_input;

/// Derive `Interpretable<I>` for a `#[wraps]` wrapper enum by delegating to
/// each wrapped statement's `Interpretable` impl.
#[proc_macro_derive(Interpretable, attributes(wraps, kirin, interpret))]
pub fn derive_interpretable(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interpretable::do_derive_interpretable(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

/// Derive `FunctionEntry<I>` for a `#[wraps]` wrapper enum. Variants marked
/// `#[callable]` delegate; all other variants report `NotCallable`.
#[proc_macro_derive(FunctionEntry, attributes(wraps, callable, kirin, interpret))]
pub fn derive_function_entry(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match function_entry::do_derive_function_entry(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}

/// Derive `InterpDispatch<I>` for a stage enum, dispatching statement
/// interpretation and function entry to each stage's language. Uses the same
/// `#[stage(...)]` attributes as `StageMeta` / `ParseDispatch`.
#[proc_macro_derive(InterpDispatch, attributes(stage))]
pub fn derive_interp_dispatch(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match interp_dispatch::generate(&ast) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// Derive `FrameBuild<V, E>` for a **concrete** total frame enum: one
/// constructor per framework walker it carries, matched by field type. Variants
/// holding dialect frames are ignored — those are injected through the dialect's
/// own `Build*` trait.
#[proc_macro_derive(FrameBuild, attributes(interpret))]
pub fn derive_frame_build(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame_build::generate(&ast, &frame_build::CONCRETE) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// Derive `AbstractFrameBuild<V, E, K>` for a **sparse forward** total frame
/// enum. `from_digraph` is emitted only when a variant carries an
/// `AbstractDiGraphFrame`; otherwise the trait's refusing default is inherited.
#[proc_macro_derive(AbstractFrameBuild, attributes(interpret))]
pub fn derive_abstract_frame_build(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame_build::generate(&ast, &frame_build::SPARSE_FORWARD) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

/// Derive `DenseFrameBuild<V, E>` for a **dense backward** total frame enum.
#[proc_macro_derive(DenseFrameBuild, attributes(interpret))]
pub fn derive_dense_frame_build(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match frame_build::generate(&ast, &frame_build::DENSE_BACKWARD) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
