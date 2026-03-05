use crate::generators::field::context::DeriveFieldIter;
use crate::generators::field::helpers::FieldInputBuilder;
use crate::prelude::*;
use crate::tokens::{
    EnumDef, EnumVariant, StructDef, StructField, TraitImpl, Method,
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

        let (_, type_generics, _) = input.generics.split_for_impl();
        let trait_impl = TraitImpl::new(impl_generics, &full_trait_path, input_name)
            .trait_generics(&trait_generics)
            .type_generics(type_generics)
            .assoc_type(trait_type_iter.clone(), quote! { #iter_name #iter_ty_generics })
            .method(Method {
                name: trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { Self::#trait_type_iter }),
                body,
            });

        let iter_def = StructDef {
            vis: quote! { pub },
            name: iter_name.clone(),
            generics: quote! { #iter_generics_tokens },
            fields: vec![StructField {
                vis: proc_macro2::TokenStream::new(),
                name: syn::Ident::new("inner", proc_macro2::Span::call_site()),
                ty: inner_type.clone(),
            }],
        };

        let iter_impl_tokens = {
            let item = &matching_item;
            let next_body = quote! { self.inner.next() };
            quote! {
                #[automatically_derived]
                impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item;
                    fn next(&mut self) -> Option<Self::Item> {
                        #next_body
                    }
                }
            }
        };

        Ok(quote! {
            #trait_impl
            #iter_def
            #iter_impl_tokens
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
                Ok(EnumVariant {
                    name: variant_ident.clone(),
                    fields: vec![inner_type.clone()],
                })
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

        let (_, type_generics, _) = input.generics.split_for_impl();
        let trait_impl = TraitImpl::new(impl_generics, &full_trait_path, input_name)
            .trait_generics(&trait_generics)
            .type_generics(type_generics)
            .assoc_type(trait_type_iter.clone(), quote! { #iter_name #iter_ty_generics })
            .method(Method {
                name: trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { Self::#trait_type_iter }),
                body,
            });

        let iter_def = EnumDef {
            vis: quote! { pub },
            name: iter_name.clone(),
            generics: quote! { #iter_generics_tokens },
            variants: variant_defs,
        };

        let next_body = quote! {
            match self {
                #(
                    Self::#variant_idents(inner) => inner.next(),
                )*
            }
        };
        let iter_impl_tokens = {
            let item = &matching_item;
            quote! {
                #[automatically_derived]
                impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item;
                    fn next(&mut self) -> Option<Self::Item> {
                        #next_body
                    }
                }
            }
        };

        Ok(quote! {
            #trait_impl
            #iter_def
            #iter_impl_tokens
        })
    }
}
