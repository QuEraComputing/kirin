use super::context::{Builder};
use crate::kirin::extra::FieldKind;
use crate::prelude::*;
use quote::{format_ident, quote};

target! {
    pub struct BuildResultName
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, BuildResultName> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> BuildResultName {
        let name = node.source_ident();
        quote! { #name }.into()
    }
}

target! {
    pub struct BuildResultModuleName
}

impl<'src, N> Compile<'src, N, BuildResultModuleName> for Builder
where
    N: WithInput<'src>,
{
    fn compile(&self, node: &N) -> BuildResultModuleName {
        let name = format_ident!(
            "{}_build_result",
            to_snake_case(node.input().ident.to_string())
        );
        quote! { #name }.into()
    }
}

target! {
    pub struct BuildResultFullPath
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, BuildResultFullPath> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> BuildResultFullPath {
        let build_result_mod: BuildResultModuleName = self.compile(node);
        let build_result_name: BuildResultName = self.compile(node);

        quote! {
            #build_result_mod::#build_result_name
        }
        .into()
    }
}

target! {
    pub struct BuildResultImpl
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, BuildResultImpl> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> BuildResultImpl {
        if node.wrapper().is_some() {
            return quote! {}.into();
        }

        let build_result_name: BuildResultName = self.compile(node);
        let fields: Vec<_> = node
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .enumerate()
            .map(|(index, f)| {
                let field_name = f.source().ident.clone().unwrap_or_else(|| format_ident!("result_{}", index, span = f.source_ident().span()));
                let f_ty = &f.source().ty;
                quote! {
                    pub #field_name: #f_ty,
                }
            })
            .collect();

        quote! {
            pub struct #build_result_name {
                pub id: Statement,
                #(#fields)*
            }

            impl From<#build_result_name> for Statement {
                fn from(value: #build_result_name) -> Self {
                    value.id
                }
            }
        }
        .into()
    }
}

target! {
    pub struct BuildResultModule
}

impl<'src> Compile<'src, Struct<'src, Builder>, BuildResultModule> for Builder {
    fn compile(&self, node: &Struct<'src, Builder>) -> BuildResultModule {
        if node.is_wrapper() {
            return quote! {}.into();
        }

        let crate_path: CratePath = self.compile(node);
        let build_result_mod: BuildResultModuleName = self.compile(node);
        let build_result_impl: BuildResultImpl = self.compile(&node.fields());

        quote! {
            pub mod #build_result_mod {
                use #crate_path::{Statement, ResultValue};
                #build_result_impl
            }
        }
        .into()
    }
}

impl<'src> Compile<'src, Enum<'src, Builder>, BuildResultModule> for Builder {
    fn compile(&self, node: &Enum<'src, Builder>) -> BuildResultModule {
        if node.marked_wraps() || node.variants().all(|v| v.is_wrapper()) {
            return quote! {}.into();
        }

        let crate_path: CratePath = self.compile(node);
        let build_result_mod: BuildResultModuleName = self.compile(node);

        let build_result_impl: Vec<BuildResultImpl> =
            node.variants().map(|v| self.compile(&v.fields())).collect();

        quote! {
            pub mod #build_result_mod {
                use #crate_path::{Statement, ResultValue};
                #(#build_result_impl)*
            }
        }
        .into()
    }
}
