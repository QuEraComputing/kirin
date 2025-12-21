use quote::{format_ident, quote};

use crate::{kirin::attrs::BuilderOptions, prelude::*};

use super::context::Builder;

target! {
    pub struct BuildFnName
}

impl<'src> Compile<'src, Struct<'src, Self>, BuildFnName> for Builder {
    fn compile(&self, node: &Struct<'src, Self>) -> BuildFnName {
        let default_name = format_ident!("new", span = node.source_ident().span());
        match &node.attrs().builder {
            Some(BuilderOptions::Named(name)) => {
                format_ident!("{}", name, span = node.source_ident().span())
                    .to_token_stream()
                    .into()
            }
            _ => quote! { #default_name }.into(),
        }
    }
}

impl<'src> Compile<'src, Variant<'_, 'src, Self>, BuildFnName> for Builder {
    fn compile(&self, node: &Variant<'_, 'src, Self>) -> BuildFnName {
        let default_name = format_ident!("op_{}", to_snake_case(node.source_ident().to_string()), span = node.source_ident().span());
        match &node.attrs().builder {
            Some(BuilderOptions::Named(name)) => {
                format_ident!("{}", name, span = node.source_ident().span())
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

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, StatementIdName> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> StatementIdName {
        let name = node.source_ident();
        let statement_id = format_ident!("{}_statement_id", name.to_string().to_lowercase(), span = name.span());
        quote! { #statement_id }.into()
    }
}
