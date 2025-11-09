extern crate proc_macro;

use kirin_derive_core::{DeriveContext, DeriveInstruction, Generate};
use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

#[proc_macro_derive(Instruction, attributes(kirin))]
pub fn derive_instruction(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::DeriveInput);
    let mut ctx = DeriveContext::new(quote! {::kirin_ir::Instruction}, ast);
    let mut instruction_impl = DeriveInstruction::new(&ctx);
    instruction_impl.generate(&mut ctx).unwrap();
    ctx.generate().into()
}
