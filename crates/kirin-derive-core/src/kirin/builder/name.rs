use quote::{format_ident, quote};

use crate::{kirin::attrs::BuilderOptions, prelude::*};

use super::context::Builder;

target! {
    pub struct BuildFnName
}

impl<'src> Compile<'src, Builder, BuildFnName> for Struct<'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> BuildFnName {
        let default_name = format_ident!("new", span = self.source_ident().span());
        match &self.attrs().builder {
            Some(BuilderOptions::Named(name)) => {
                format_ident!("{}", name, span = self.source_ident().span())
                    .to_token_stream()
                    .into()
            }
            _ => quote! { #default_name }.into(),
        }
    }
}

impl<'src> Compile<'src, Builder, BuildFnName> for Variant<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> BuildFnName {
        let default_name = format_ident!(
            "op_{}",
            to_snake_case(self.source_ident().to_string()),
            span = self.source_ident().span()
        );
        match &self.attrs().builder {
            Some(BuilderOptions::Named(name)) => {
                format_ident!("{}", name, span = self.source_ident().span())
                    .to_token_stream()
                    .into()
            }
            _ => quote! { #default_name }.into(),
        }
    }
}

target! {
    pub struct StatementIdName
}

impl<'src> Compile<'src, Builder, StatementIdName> for Fields<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> StatementIdName {
        let name = self.source_ident();
        let statement_id = format_ident!(
            "{}_statement_id",
            name.to_string().to_lowercase(),
            span = name.span()
        );
        quote! { #statement_id }.into()
    }
}
