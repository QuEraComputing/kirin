use super::{CallSemanticsLayout, DeriveCallSemantics};
use kirin_derive_core::prelude::*;
use quote::quote;

impl<'ir> Emit<'ir, CallSemanticsLayout> for DeriveCallSemantics {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<CallSemanticsLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let interp_crate = self.interpreter_crate_path();
        let ir_crate = self.ir_crate_path(input);
        let type_name = &input.core.name;
        let generics = add_interpreter_param(&input.core.generics);
        let (impl_generics, _, _) = generics.split_for_impl();
        let (_, ty_generics, where_clause) = input.core.generics.split_for_impl();

        if info.is_wrapper {
            let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
            let pattern = &info.pattern;
            let binding = info.wrapper_binding.as_ref().unwrap();

            Ok(quote! {
                impl #impl_generics #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>
                    for #type_name #ty_generics
                where
                    __CallSemI: #interp_crate::Interpreter,
                    __CallSemI::Error: From<#interp_crate::InterpreterError>,
                    #wrapper_ty: #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>,
                    #where_clause
                {
                    type Result = <#wrapper_ty as #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>>::Result;

                    fn call_semantics(
                        &self,
                        interpreter: &mut __CallSemI,
                        callee: #ir_crate::SpecializedFunction,
                        args: &[__CallSemI::Value],
                    ) -> Result<Self::Result, __CallSemI::Error> {
                        let Self #pattern = self;
                        #binding.call_semantics(interpreter, callee, args)
                    }
                }
            })
        } else {
            Ok(quote! {
                impl #impl_generics #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>
                    for #type_name #ty_generics
                where
                    __CallSemI: #interp_crate::Interpreter,
                    __CallSemI::Error: From<#interp_crate::InterpreterError>,
                    #where_clause
                {
                    type Result = __CallSemI::Value;

                    fn call_semantics(
                        &self,
                        _interpreter: &mut __CallSemI,
                        _callee: #ir_crate::SpecializedFunction,
                        _args: &[__CallSemI::Value],
                    ) -> Result<Self::Result, __CallSemI::Error> {
                        Err(#interp_crate::InterpreterError::MissingEntry.into())
                    }
                }
            })
        }
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<CallSemanticsLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let interp_crate = self.interpreter_crate_path();
        let ir_crate = self.ir_crate_path(input);
        let type_name = &input.core.name;
        let generics = add_interpreter_param(&input.core.generics);
        let (impl_generics, _, _) = generics.split_for_impl();
        let (_, ty_generics, where_clause) = input.core.generics.split_for_impl();

        // Determine if #[callable] is used anywhere (enum-level or any variant).
        let any_callable = input.callable_all
            || self
                .statements
                .values()
                .any(|info| info.is_callable);

        let mut wrapper_types: Vec<&syn::Type> = Vec::new();
        let mut match_arms = Vec::new();

        for variant in &data.variants {
            let info = self.statement_info(variant)?;
            let variant_name = &info.name;
            let pattern = &info.pattern;

            // A variant forwards call_semantics if:
            // - No #[callable] used anywhere: fall back to #[wraps] (backward compat)
            // - #[callable] used: only callable wrappers forward
            let is_call_wrapper = if any_callable {
                info.is_wrapper && info.is_callable
            } else {
                info.is_wrapper
            };

            if is_call_wrapper {
                let wrapper_ty = info.wrapper_ty.as_ref().unwrap();
                wrapper_types.push(wrapper_ty);
                let binding = info.wrapper_binding.as_ref().unwrap();

                match_arms.push(quote! {
                    Self::#variant_name #pattern => #binding.call_semantics(interpreter, callee, args)
                });
            } else if info.pattern.is_empty() {
                match_arms.push(quote! {
                    Self::#variant_name => Err(#interp_crate::InterpreterError::MissingEntry.into())
                });
            } else {
                match_arms.push(quote! {
                    Self::#variant_name #pattern => Err(#interp_crate::InterpreterError::MissingEntry.into())
                });
            }
        }

        let result_type = if let Some(first_wrapper) = wrapper_types.first() {
            quote! { <#first_wrapper as #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>>::Result }
        } else {
            quote! { __CallSemI::Value }
        };

        let where_bounds: Vec<proc_macro2::TokenStream> = wrapper_types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                if i == 0 {
                    quote! {
                        #ty: #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>,
                    }
                } else {
                    quote! {
                        #ty: #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics, Result = #result_type>,
                    }
                }
            })
            .collect();

        Ok(quote! {
            impl #impl_generics #interp_crate::CallSemantics<__CallSemI, #type_name #ty_generics>
                for #type_name #ty_generics
            where
                __CallSemI: #interp_crate::Interpreter,
                __CallSemI::Error: From<#interp_crate::InterpreterError>,
                #(#where_bounds)*
                #where_clause
            {
                type Result = #result_type;

                fn call_semantics(
                    &self,
                    interpreter: &mut __CallSemI,
                    callee: #ir_crate::SpecializedFunction,
                    args: &[__CallSemI::Value],
                ) -> Result<Self::Result, __CallSemI::Error> {
                    match self {
                        #(#match_arms,)*
                    }
                }
            }
        })
    }
}

fn add_interpreter_param(base: &syn::Generics) -> syn::Generics {
    let mut generics = base.clone();
    let param: syn::TypeParam = syn::parse_quote! { __CallSemI };
    generics.params.push(syn::GenericParam::Type(param));
    generics
}
