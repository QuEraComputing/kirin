use quote::quote;

use crate::kirin::builder::result::ResultNames;
use crate::kirin::{builder::initialization::InitializationHead, extra::FieldKind};
use crate::prelude::*;

use super::{
    build_result::BuildResultFullPath,
    context::Builder,
    initialization::Initialization,
    input::{InputSignature, LetNameEqInput},
    name::{BuildFnName, StatementIdName},
    result::LetNameEqResultValue,
};

target! {
    pub struct BuildFnImpl
}

impl<'src> Compile<'src, Builder, BuildFnImpl> for Struct<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildFnImpl {
        if self.is_wrapper() {
            return quote! {}.into();
        }

        let name = self.source_ident();
        let build_fn_name: BuildFnName = self.compile(ctx);
        let type_lattice = &self.attrs().type_lattice;
        let crate_path: CratePath = self.compile(ctx);
        let (impl_generics, ty_generics, where_clause) = self.source().generics.split_for_impl();
        let inputs: Inputs = self.fields().compile(ctx);
        let build_result_path: BuildResultFullPath = self.fields().compile(ctx);
        let body: BuildFnBody = self.fields().compile(ctx);

        quote! {
            #[automatically_derived]
            impl #impl_generics #name #ty_generics #where_clause {
                pub fn #build_fn_name<Lang>(context: &mut #crate_path::Context<Lang>, #inputs) -> #build_result_path
                where
                    Lang: #crate_path::Dialect + From<#name #ty_generics>,
                    Lang::TypeLattice: From<#type_lattice>
                #body
            }
        }
        .into()
    }
}

impl<'src> Compile<'src, Builder, BuildFnImpl> for Enum<'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildFnImpl {
        if self.marked_wraps() || self.variants().all(|v| v.is_wrapper()) {
            return quote! {}.into();
        }

        let name = self.source_ident();
        let (impl_generics, ty_generics, where_clause) = self.generics().split_for_impl();
        let type_lattice = &self.attrs().type_lattice;

        let crate_path: CratePath = self.compile(ctx);
        let functions: Vec<_> = self.variants().map(|v| {
            if v.is_wrapper() {
                return quote! {};
            }

            let build_fn_name: BuildFnName = v.compile(ctx);
            let build_result_path: BuildResultFullPath = v.fields().compile(ctx);
            let inputs: Inputs = v.fields().compile(ctx);
            let body: BuildFnBody = v.fields().compile(ctx);
            quote! {
                pub fn #build_fn_name<Lang>(context: &mut #crate_path::Context<Lang>, #inputs) -> #build_result_path
                where
                    Lang: #crate_path::Dialect + From<#name #ty_generics>,
                    Lang::TypeLattice: From<#type_lattice>
                #body
            }
        }).collect();

        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#functions)*
            }
        }
        .into()
    }
}

target! {
    pub struct Inputs
}

impl<'src> Compile<'src, Builder, Inputs> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> Inputs {
        let inputs: Vec<InputSignature> = self
            .iter()
            .filter(|f| {
                f.attrs().default.is_none() && !matches!(&f.extra().kind, FieldKind::ResultValue)
            })
            .map(|f| f.compile(ctx))
            .collect();
        quote! { #(#inputs),* }.into()
    }
}

target! {
    pub struct BuildFnBody
}

impl<'src> Compile<'src, Builder, BuildFnBody> for Fields<'_, 'src, Builder> {
    fn compile(&self, ctx: &Builder) -> BuildFnBody {
        let build_result_path: BuildResultFullPath = self.compile(ctx);
        let statement_id: StatementIdName = self.compile(ctx);
        let let_name_eq_input: Vec<LetNameEqInput> = self
            .iter()
            .filter(|f| !matches!(f.extra().kind, FieldKind::ResultValue))
            .map(|f| f.compile(ctx))
            .collect();
        let let_name_eq_result: LetNameEqResultValue = self.compile(ctx);

        let head_self: InitializationHead = self.compile(ctx);
        let initialization: Initialization = self.compile(ctx);
        let result_names: ResultNames = self.compile(ctx);

        quote! {{
            let #statement_id = context.statement_arena().next_id();
            #(#let_name_eq_input)*
            #let_name_eq_result

            context
                .statement()
                .definition(#head_self #initialization)
                .new();

            #build_result_path {
                id: #statement_id,
                #(#result_names),*
            }
        }}
        .into()
    }
}
