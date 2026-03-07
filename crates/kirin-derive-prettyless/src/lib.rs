extern crate proc_macro;

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod generate;

#[proc_macro_derive(RenderStage, attributes(stage, pretty))]
pub fn derive_render_stage(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    match generate::generate(&ast) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(err) => TokenStream::from(err.into_compile_error()),
    }
}
