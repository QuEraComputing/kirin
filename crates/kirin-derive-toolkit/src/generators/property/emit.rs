use crate::generators::property::context::DeriveProperty;
use crate::prelude::*;
use crate::tokens::TraitImpl;
use crate::tokens::Method;
use quote::quote;

impl<'ir> Emit<'ir, StandardLayout> for DeriveProperty {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let trait_path = self.full_trait_path(input);
        let input_name = &input.core.name;
        let value_type = &self.value_type;

        let self_arg = quote! { &self };
        let body = if info.is_wrapper {
            let unpack = &info.pattern;
            let value_expr = &info.value_expr;
            quote! {
                let Self #unpack = self;
                #value_expr
            }
        } else {
            let value_expr = &info.value_expr;
            quote! { #value_expr }
        };

        let trait_impl = TraitImpl::new(input.core.generics.clone(), &trait_path, input_name)
            .method(Method {
                name: self.trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { #value_type }),
                body,
            });

        Ok(quote! { #trait_impl })
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let trait_path = self.full_trait_path(input);
        let input_name = &input.core.name;
        let value_type = &self.value_type;

        let variant_patterns = data
            .variants
            .iter()
            .map(|v| {
                let info = self.statement_info(v)?;
                let name = &info.name;
                if info.pattern_empty {
                    Ok(quote! { Self::#name })
                } else {
                    let unpack = &info.pattern;
                    Ok(quote! { Self::#name #unpack })
                }
            })
            .collect::<darling::Result<Vec<_>>>()?;
        let variant_exprs = data
            .variants
            .iter()
            .map(|v| {
                let info = self.statement_info(v)?;
                Ok(info.value_expr.clone())
            })
            .collect::<darling::Result<Vec<_>>>()?;

        let self_arg = quote! { &self };
        let body = quote! {
            match self {
                #(
                    #variant_patterns => #variant_exprs
                ),*
            }
        };

        let trait_impl = TraitImpl::new(input.core.generics.clone(), &trait_path, input_name)
            .method(Method {
                name: self.trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { #value_type }),
                body,
            });

        Ok(quote! { #trait_impl })
    }
}
