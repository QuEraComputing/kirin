use proc_macro2::TokenStream;
use quote::ToTokens;

use super::compile::{Alt, Compile};
use crate::ir::{Input, Layout, ScanInto};

pub trait Emit<'src>:
    Layout + Compile<'src, Input<'src, Self>, Alt<Self::StructImpl, Self::EnumImpl>> + ScanInto + Sized
{
    type StructImpl;
    type EnumImpl;

    fn emit(&mut self, input: &'src syn::DeriveInput) -> TokenStream {
        self.scan(input)
            .and_then(|node| {
                let fi: Alt<Self::StructImpl, Self::EnumImpl> = self.compile(&node);
                Ok(fi.into_token_stream())
            })
            .unwrap_or_else(|e| e.to_compile_error())
    }

    #[cfg(feature = "debug")]
    fn print(&mut self, input: &'src syn::DeriveInput) -> String {
        use crate::debug::rustfmt;
        let source = self.emit(input).to_string();
        match syn::parse_file(&source) {
            Ok(_) => rustfmt(source),
            Err(_) => {
                rustfmt(source);
                panic!("Failed to parse generated code")
            }
        }
    }
}
