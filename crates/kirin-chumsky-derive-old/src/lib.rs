extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(WithRecursiveChumskyParser, attributes(kirin, chumsky, wraps))]
pub fn derive_with_recursive_chumsky_parser(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let ir_input =
        match kirin_derive_core_2::ir::Input::<kirin_chumsky_format::ChumskyLayout>::from_derive_input(&ast) {
            Ok(ir) => ir,
            Err(err) => return err.write_errors().into(),
        };

    kirin_chumsky_format::parser::DeriveChumskyParser::new(&ir_input)
        .generate(&ir_input)
        .into()
}

/// Backwards-compatible alias for older derives.
#[proc_macro_derive(HasParser, attributes(kirin, chumsky, wraps))]
pub fn derive_has_parser(input: TokenStream) -> TokenStream {
    derive_with_recursive_chumsky_parser(input)
}
