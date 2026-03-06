extern crate proc_macro;

mod attrs;
mod codegen;
mod field_kind;
mod format;
mod input;
mod validation;
mod visitor;

use attrs::{ChumskyFieldAttrs, ChumskyGlobalAttrs, ChumskyStatementAttrs, PrettyGlobalAttrs};
use codegen::{GenerateAST, GenerateEmitIR, GenerateHasDialectParser, GeneratePrettyPrint};
use input::{parse_derive_input, parse_pretty_derive_input};

use kirin_derive_toolkit::ir::Layout;
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// The layout for chumsky derive macros.
#[derive(Debug, Clone)]
pub(crate) struct ChumskyLayout;

impl Layout for ChumskyLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = ChumskyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}

/// The layout for the `PrettyPrint` derive macro.
///
/// Reuses `ChumskyStatementAttrs` and `ChumskyFieldAttrs` for format strings,
/// but uses `PrettyGlobalAttrs` for the `#[pretty(crate = ...)]` attribute.
#[derive(Debug, Clone)]
pub(crate) struct PrettyPrintLayout;

impl Layout for PrettyPrintLayout {
    type StatementExtra = ();
    type ExtraGlobalAttrs = PrettyGlobalAttrs;
    type ExtraStatementAttrs = ChumskyStatementAttrs;
    type ExtraFieldAttrs = ChumskyFieldAttrs;
}

#[proc_macro_derive(HasParser, attributes(kirin, chumsky, wraps))]
pub fn derive_has_parser(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input = match parse_derive_input(&ast) {
        Ok(ir) => ir,
        Err(err) => return err.write_errors().into(),
    };

    let ast_generator = GenerateAST::new(&ir_input);
    let parser_generator = GenerateHasDialectParser::new(&ir_input);
    let emit_generator = GenerateEmitIR::new(&ir_input);

    let ast_tokens = ast_generator.generate(&ir_input);
    let parser_tokens = parser_generator.generate(&ir_input);
    let emit_tokens = emit_generator.generate(&ir_input);

    let output = quote! {
        #ast_tokens
        #parser_tokens
        #emit_tokens
    };

    output.into()
}

#[proc_macro_derive(PrettyPrint, attributes(kirin, chumsky, wraps, pretty))]
pub fn derive_pretty_print(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);

    let ir_input = match parse_pretty_derive_input(&ast) {
        Ok(ir) => ir,
        Err(err) => return err.write_errors().into(),
    };

    let generator = GeneratePrettyPrint::new(&ir_input);
    generator.generate(&ir_input).into()
}
