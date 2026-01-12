use super::context::Builder;
use crate::kirin::{builder::result::ResultNames, extra::FieldKind};
use crate::prelude::*;
use quote::{format_ident, quote};

target! {
    pub struct BuildResultName
}

impl<'src> Compile<'src, Builder, BuildResultName> for Fields<'_, 'src, Builder> {
    fn compile(&self, _ctx: &Builder) -> BuildResultName {
        let name = self.source_ident();
        quote! { #name }.into()
    }
}

target! {
    pub struct BuildResultModuleName
}

impl<'src, N> Compile<'src, Builder, BuildResultModuleName> for N
where
    N: WithInput<'src>,
{
    fn compile(&self, _ctx: &Builder) -> BuildResultModuleName {
        let name = format_ident!(
            "{}_build_result",
            to_snake_case(self.input().ident.to_string())
        );
        quote! { #name }.into()
    }
}

target! {
    pub struct BuildResultFullPath
}

impl<'src> Compile<'src, Builder, BuildResultFullPath> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildResultFullPath {
        let build_result_mod: BuildResultModuleName = self.compile(ctx);
        let build_result_name: BuildResultName = self.compile(ctx);

        quote! {
            #build_result_mod::#build_result_name
        }
        .into()
    }
}

target! {
    pub struct BuildResultImpl
}

impl<'src> Compile<'src, Builder, BuildResultImpl> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildResultImpl {
        if self.wrapper().is_some() {
            return quote! {}.into();
        }

        let build_result_name: BuildResultName = self.compile(ctx);
        let result_names: ResultNames = self.compile(ctx);
        let fields: Vec<_> = self
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .zip(result_names)
            .map(|(f, field_name)| {
                let f_ty = &f.source().ty;
                quote! {
                    pub #field_name: #f_ty,
                }
            })
            .collect();

        quote! {
            #[automatically_derived]
            pub struct #build_result_name {
                pub id: Statement,
                #(#fields)*
            }

            #[automatically_derived]
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

impl<'src> Compile<'src, Builder, BuildResultModule> for Struct<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildResultModule {
        if self.is_wrapper() {
            return quote! {}.into();
        }

        let crate_path: CratePath = self.compile(ctx);
        let build_result_mod: BuildResultModuleName = self.compile(ctx);
        let build_result_impl: BuildResultImpl = self.fields().compile(ctx);

        quote! {
            #[automatically_derived]
            pub mod #build_result_mod {
                use #crate_path::{Statement, ResultValue};
                #build_result_impl
            }
        }
        .into()
    }
}

impl<'src> Compile<'src, Builder, BuildResultModule> for Enum<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildResultModule {
        if self.marked_wraps() || self.variants().all(|v| v.is_wrapper()) {
            return quote! {}.into();
        }

        let crate_path: CratePath = self.compile(ctx);
        let build_result_mod: BuildResultModuleName = self.compile(ctx);

        let build_result_impl: Vec<BuildResultImpl> =
            self.variants().map(|v| v.fields().compile(ctx)).collect();
        quote! {
            #[automatically_derived]
            pub mod #build_result_mod {
                use #crate_path::{Statement, ResultValue};
                #(#build_result_impl)*
            }
        }
        .into()
    }
}
