use quote::{format_ident, quote};

use crate::kirin::extra::FieldKind;
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

impl<'src> Compile<'src, Struct<'src, Builder>, BuildFnImpl> for Builder {
    fn compile(&self, node: &Struct<'src, Builder>) -> BuildFnImpl {
        if node.is_wrapper() {
            return quote! {}.into();
        }

        let name = node.source_ident();
        let build_fn_name: BuildFnName = self.compile(node);
        let type_lattice = &node.attrs().type_lattice;
        let crate_path: CratePath = self.compile(node);
        let (impl_generics, ty_generics, where_clause) = node.source().generics.split_for_impl();
        let inputs: Inputs = self.compile(&node.fields());
        let build_result_path: BuildResultFullPath = self.compile(&node.fields());
        let body: BuildFnBody = self.compile(&node.fields());

        quote! {
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

impl<'src> Compile<'src, Enum<'src, Builder>, BuildFnImpl> for Builder {
    fn compile(&self, node: &Enum<'src, Builder>) -> BuildFnImpl {
        if node.marked_wraps() || node.variants().all(|v| v.is_wrapper()) {
            return quote! {}.into();
        }

        let name = node.source_ident();
        let (impl_generics, ty_generics, where_clause) = node.generics().split_for_impl();
        let type_lattice = &node.attrs().type_lattice;

        let crate_path: CratePath = self.compile(node);
        let functions: Vec<_> = node.variants().map(|v| {
            if v.is_wrapper() {
                return quote! {};
            }

            let build_fn_name: BuildFnName = self.compile(&v);
            let build_result_path: BuildResultFullPath = self.compile(&v.fields());
            let inputs: Inputs = self.compile(&v.fields());
            let body: BuildFnBody = self.compile(&v.fields());
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

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, Inputs> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> Inputs {
        let inputs: Vec<InputSignature> = node
            .iter()
            .filter(|f| {
                f.attrs().default.is_none() && !matches!(&f.extra().kind, FieldKind::ResultValue)
            })
            .map(|f| self.compile(&f))
            .collect();
        quote! { #(#inputs),* }.into()
    }
}

target! {
    pub struct BuildFnBody
}

impl<'src> Compile<'src, Fields<'_, 'src, Builder>, BuildFnBody> for Builder {
    fn compile(&self, node: &Fields<'_, 'src, Builder>) -> BuildFnBody {
        let build_result_path: BuildResultFullPath = self.compile(node);
        let statement_id: StatementIdName = self.compile(node);
        let let_name_eq_input: Vec<LetNameEqInput> = node
            .iter()
            .filter(|f| !matches!(f.extra().kind, FieldKind::ResultValue))
            .map(|f| self.compile(&f))
            .collect();
        let let_name_eq_result: LetNameEqResultValue = self.compile(node);
        let initialization: Initialization = self.compile(node);
        let statement = format_ident!("{}_statement", node.source_ident());
        let result_names: Vec<_> = node
            .iter()
            .filter(|f| matches!(f.extra().kind, FieldKind::ResultValue))
            .map(|f| f.source_ident())
            .collect();

        quote! {{
            let #statement_id = context.statement_arena().next_id();
            #(#let_name_eq_input)*
            #let_name_eq_result

            let #statement = context
                .statement()
                .definition(Self #initialization)
                .new();

            #build_result_path {
                id: #statement_id,
                #(#result_names),*
            }
        }}
        .into()
    }
}
