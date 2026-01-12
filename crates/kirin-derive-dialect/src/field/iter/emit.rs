use crate::field::iter::context::DeriveFieldIter;
use crate::field::iter::helpers::FieldInputBuilder;
use kirin_derive_core_2::prelude::*;
use kirin_derive_core_2::tokens::{
    IterEnumDefTokens, IterStructDefTokens, IteratorImplTokens, TraitImplTokens, VariantDefTokens,
};
use quote::quote;

impl<'ir> Emit<'ir, StandardLayout> for DeriveFieldIter {
    fn emit_struct(
        &mut self,
        data: &'ir ir::DataStruct<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let info = self.statement_info(&data.0)?;
        let input_name = &input.name;
        let input_builder = FieldInputBuilder::new(self, input);
        let full_trait_path = input_builder.full_trait_path();
        let iter_name = input_builder.iter_type_name();
        let matching_item = input_builder.matching_item();
        let trait_generics = input_builder.trait_generics();
        let iter_generics = input_builder.iter_generics(info.is_wrapper);
        let iter_generics_tokens = iter_generics.clone();
        let trait_lifetime = &self.trait_lifetime;
        let trait_method = &self.trait_method;
        let trait_type_iter = &self.trait_type_iter;

        let impl_generics = input_builder.add_trait_lifetime(&input.generics);
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        let unpack = &info.pattern;
        let iter_expr = &info.iter_expr;
        let body = if info.pattern_empty {
            quote! {
                #iter_name {
                    inner: #iter_expr,
                }
            }
        } else {
            quote! {
                let Self #unpack = self;
                #iter_name {
                    inner: #iter_expr,
                }
            }
        };

        let inner_type = &info.inner_type;

        let self_arg = if self.mutable {
            quote! { &#trait_lifetime mut self }
        } else {
            quote! { &#trait_lifetime self }
        };

        let trait_impl = TraitImplTokens::builder()
            .impl_and_type_generics(&impl_generics, &input.generics)
            .trait_path(&full_trait_path)
            .trait_generics(trait_generics)
            .type_name(input_name)
            .assoc_type_ident(trait_type_iter.clone())
            .assoc_type(quote! { #iter_name #iter_ty_generics })
            .method_name(trait_method.clone())
            .self_arg(self_arg)
            .body(body)
            .build();

        let iter_def = IterStructDefTokens::builder()
            .name(&iter_name)
            .generics(iter_generics_tokens)
            .inner_type(inner_type)
            .build();

        let iter_impl = IteratorImplTokens::builder()
            .impl_generics(iter_impl_generics)
            .name(&iter_name)
            .type_generics(iter_ty_generics)
            .where_clause(iter_where_clause)
            .item(matching_item)
            .next_body(quote! { self.inner.next() })
            .build();

        Ok(quote! {
            #trait_impl
            #iter_def
            #iter_impl
        })
    }

    fn emit_enum(
        &mut self,
        data: &'ir ir::DataEnum<StandardLayout>,
    ) -> darling::Result<proc_macro2::TokenStream> {
        let input = self.input_ctx()?;
        let input_name = &input.name;
        let input_builder = FieldInputBuilder::new(self, input);
        let full_trait_path = input_builder.full_trait_path();
        let iter_name = input_builder.iter_type_name();
        let matching_item = input_builder.matching_item();
        let trait_generics = input_builder.trait_generics();
        let needs_input_generics = data.variants.iter().any(|v| v.wraps.is_some());
        let iter_generics = input_builder.iter_generics(needs_input_generics);
        let iter_generics_tokens = iter_generics.clone();
        let trait_lifetime = &self.trait_lifetime;
        let trait_method = &self.trait_method;
        let trait_type_iter = &self.trait_type_iter;

        let impl_generics = input_builder.add_trait_lifetime(&input.generics);
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        let variant_idents: Vec<_> = data.variants.iter().map(|v| v.name.clone()).collect();
        let variant_defs: Vec<_> = data
            .variants
            .iter()
            .map(|v| {
                let info = self.statement_info(v)?;
                let variant_ident = &info.name;
                let inner_type = &info.inner_type;
                Ok(VariantDefTokens::builder()
                    .name(variant_ident.clone())
                    .inner_type(inner_type)
                    .build())
            })
            .collect::<darling::Result<Vec<_>>>()?;

        let variant_exprs: Vec<_> = data
            .variants
            .iter()
            .map(|v| {
                let info = self.statement_info(v)?;
                let variant_ident = &info.name;
                let iter_expr = &info.iter_expr;
                if info.pattern_empty {
                    Ok(quote! { Self::#variant_ident => #iter_name::#variant_ident(#iter_expr) })
                } else {
                    let unpack = &info.pattern;
                    Ok(quote! {
                        Self::#variant_ident #unpack => #iter_name::#variant_ident(#iter_expr)
                    })
                }
            })
            .collect::<darling::Result<Vec<_>>>()?;

        let self_arg = if self.mutable {
            quote! { &#trait_lifetime mut self }
        } else {
            quote! { &#trait_lifetime self }
        };
        let body = quote! {
            match self {
                #(
                    #variant_exprs
                ),*
            }
        };

        let trait_impl = TraitImplTokens::builder()
            .impl_and_type_generics(&impl_generics, &input.generics)
            .trait_path(&full_trait_path)
            .trait_generics(trait_generics)
            .type_name(input_name)
            .assoc_type_ident(trait_type_iter.clone())
            .assoc_type(quote! { #iter_name #iter_ty_generics })
            .method_name(trait_method.clone())
            .self_arg(self_arg)
            .body(body)
            .build();
        let iter_def = IterEnumDefTokens::builder()
            .name(&iter_name)
            .generics(iter_generics_tokens)
            .variants(variant_defs)
            .build();
        let iter_impl = IteratorImplTokens::builder()
            .impl_generics(iter_impl_generics)
            .name(&iter_name)
            .type_generics(iter_ty_generics)
            .where_clause(iter_where_clause)
            .item(matching_item)
            .next_body(quote! {
                match self {
                    #(
                        Self::#variant_idents(inner) => inner.next(),
                    )*
                }
            })
            .build();

        Ok(quote! {
            #trait_impl
            #iter_def
            #iter_impl
        })
    }
}
