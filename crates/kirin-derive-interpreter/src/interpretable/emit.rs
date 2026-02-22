use super::DeriveInterpretable;
use kirin_derive_core::prelude::*;
use quote::quote;

impl<'ir> Emit<'ir, StandardLayout> for DeriveInterpretable {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let interp_crate = self.interpreter_crate_path();
        let type_name = &input.core.name;
        let generics = add_interpreter_params(&input.core.generics);
        let (impl_generics, _, _) = generics.split_for_impl();
        let (_, ty_generics, where_clause) = input.core.generics.split_for_impl();

        if info.is_wrapper {
            let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
            let pattern = &info.pattern;
            let binding = info.wrapper_binding.as_ref().unwrap();

            Ok(quote! {
                impl #impl_generics #interp_crate::Interpretable<__InterpI, __InterpL>
                    for #type_name #ty_generics
                where
                    __InterpI: #interp_crate::Interpreter,
                    __InterpL: ::kirin_ir::Dialect,
                    #wrapper_ty: #interp_crate::Interpretable<__InterpI, __InterpL>,
                    #where_clause
                {
                    fn interpret(
                        &self,
                        interpreter: &mut __InterpI,
                    ) -> Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> {
                        let Self #pattern = self;
                        #binding.interpret(interpreter)
                    }
                }
            })
        } else {
            Err(darling::Error::custom(format!(
                "Cannot derive `Interpretable` for struct `{}` without `#[wraps]`. \
                 Either implement `Interpretable` manually, or wrap an inner type with `#[wraps]`.",
                type_name
            ))
            .with_span(&data.0.name))
        }
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let interp_crate = self.interpreter_crate_path();
        let type_name = &input.core.name;
        let generics = add_interpreter_params(&input.core.generics);
        let (impl_generics, _, _) = generics.split_for_impl();
        let (_, ty_generics, where_clause) = input.core.generics.split_for_impl();

        // Check that ALL variants are wrappers
        let mut non_wrapper_variants = Vec::new();
        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            if !info.is_wrapper {
                non_wrapper_variants.push(info.name.clone());
            }
        }

        if !non_wrapper_variants.is_empty() {
            let names: Vec<String> = non_wrapper_variants.iter().map(|n| n.to_string()).collect();
            return Err(darling::Error::custom(format!(
                "Cannot derive `Interpretable` for enum `{}`: variant(s) {} are not `#[wraps]`. \
                 Either implement `Interpretable` manually, or split each variant as a separate \
                 struct, impl `Interpretable` on it, then wrap it with `#[wraps]`.",
                type_name,
                names.join(", "),
            ))
            .with_span(&type_name));
        }

        let mut wrapper_types: Vec<&syn::Type> = Vec::new();
        let mut match_arms = Vec::new();

        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            let variant_name = &info.name;
            let pattern = &info.pattern;
            let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
            wrapper_types.push(wrapper_ty);
            let binding = info.wrapper_binding.as_ref().unwrap();

            match_arms.push(quote! {
                Self::#variant_name #pattern => #binding.interpret(interpreter)
            });
        }

        let where_bounds: Vec<proc_macro2::TokenStream> = wrapper_types
            .iter()
            .map(|ty| {
                quote! {
                    #ty: #interp_crate::Interpretable<__InterpI, __InterpL>,
                }
            })
            .collect();

        Ok(quote! {
            impl #impl_generics #interp_crate::Interpretable<__InterpI, __InterpL>
                for #type_name #ty_generics
            where
                __InterpI: #interp_crate::Interpreter,
                __InterpL: ::kirin_ir::Dialect,
                #(#where_bounds)*
                #where_clause
            {
                fn interpret(
                    &self,
                    interpreter: &mut __InterpI,
                ) -> Result<#interp_crate::Continuation<__InterpI::Value, __InterpI::Ext>, __InterpI::Error> {
                    match self {
                        #(#match_arms,)*
                    }
                }
            }
        })
    }
}

fn add_interpreter_params(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    let interp_param: syn::TypeParam = syn::parse_quote! { __InterpI };
    let lang_param: syn::TypeParam = syn::parse_quote! { __InterpL };
    generics.params.push(syn::GenericParam::Type(interp_param));
    generics.params.push(syn::GenericParam::Type(lang_param));
    generics
}
