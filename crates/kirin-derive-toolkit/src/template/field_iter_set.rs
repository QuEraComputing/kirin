use crate::context::DeriveContext;
use crate::ir::{self, StandardLayout};
use crate::misc::{from_str, to_camel_case};
use crate::tokens::{EnumDef, EnumVariant, Method, StructDef, StructField, TraitImpl};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};

use super::Template;
use super::method_pattern::field_collection::FieldCollection;
use super::trait_impl::FieldIterConfig;

/// A composite template set for field iterator traits.
///
/// Generates three pieces:
/// 1. Trait impl (e.g., `impl HasArguments for Type`)
/// 2. Iterator struct/enum definition
/// 3. `Iterator` impl for the generated iterator type
pub struct FieldIterTemplateSet {
    collection: FieldCollection,
    iter_name_suffix: syn::Ident,
}

impl FieldIterTemplateSet {
    pub fn new(config: FieldIterConfig, default_crate_path: &str, trait_lifetime: &str) -> Self {
        let trait_method: syn::Ident = from_str(config.trait_method);
        let iter_name_suffix = format_ident!("{}Iter", to_camel_case(trait_method.to_string()));

        Self {
            collection: FieldCollection {
                field_kind: config.kind,
                mutable: config.mutable,
                default_crate_path: from_str(default_crate_path),
                trait_path: from_str(config.trait_name),
                trait_lifetime: from_str(trait_lifetime),
                trait_method,
                trait_type_iter: from_str(config.trait_type_iter),
                matching_type: from_str(config.matching_type),
            },
            iter_name_suffix,
        }
    }

    fn iter_type_name(&self, ctx: &DeriveContext<'_, StandardLayout>) -> syn::Ident {
        format_ident!("{}{}", ctx.meta.name, self.iter_name_suffix)
    }

    fn trait_generics(&self) -> syn::Generics {
        self.collection.trait_generics()
    }

    fn add_trait_lifetime(&self, generics: &syn::Generics) -> syn::Generics {
        let mut generics = generics.clone();
        let lifetime_ident = &self.collection.trait_lifetime.ident;
        let has_lifetime = generics
            .lifetimes()
            .any(|lt| lt.lifetime.ident == *lifetime_ident);
        if !has_lifetime {
            generics.params.insert(
                0,
                syn::GenericParam::Lifetime(syn::LifetimeParam::new(
                    self.collection.trait_lifetime.clone(),
                )),
            );
        }
        generics
    }

    fn iter_generics(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        needs_input_generics: bool,
    ) -> syn::Generics {
        if needs_input_generics {
            self.add_trait_lifetime(&ctx.meta.generics)
        } else {
            self.trait_generics()
        }
    }

    fn emit_struct(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &ir::DataStruct<StandardLayout>,
    ) -> darling::Result<Vec<TokenStream>> {
        let stmt_ctx = ctx
            .statements
            .get(&data.0.name.to_string())
            .ok_or_else(|| darling::Error::custom("missing statement context"))?;

        let full_trait_path = self.collection.full_trait_path(ctx);
        let iter_name = self.iter_type_name(ctx);
        let matching_item = self.collection.matching_item(ctx);
        let trait_generics = self.trait_generics();
        let iter_generics = self.iter_generics(ctx, stmt_ctx.is_wrapper);
        let iter_generics_tokens = iter_generics.clone();
        let trait_lifetime = &self.collection.trait_lifetime;
        let trait_method = &self.collection.trait_method;
        let trait_type_iter = &self.collection.trait_type_iter;
        let input_name = &ctx.meta.name;

        let impl_generics = self.add_trait_lifetime(&ctx.meta.generics);
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        let (iter_expr, inner_type) = self.collection.statement_iter(ctx, stmt_ctx);
        let pattern = &stmt_ctx.pattern;
        let body = if stmt_ctx.pattern.is_empty() {
            quote! {
                #iter_name {
                    inner: #iter_expr,
                }
            }
        } else {
            quote! {
                let Self #pattern = self;
                #iter_name {
                    inner: #iter_expr,
                }
            }
        };

        let self_arg = if self.collection.mutable {
            quote! { &#trait_lifetime mut self }
        } else {
            quote! { &#trait_lifetime self }
        };

        let (_, type_generics, _) = ctx.meta.generics.split_for_impl();
        let trait_impl = TraitImpl::new(impl_generics, &full_trait_path, input_name)
            .trait_generics(&trait_generics)
            .type_generics(type_generics)
            .assoc_type(
                trait_type_iter.clone(),
                quote! { #iter_name #iter_ty_generics },
            )
            .method(Method {
                name: trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { Self::#trait_type_iter }),
                body,
                generics: None,
                method_where_clause: None,
            });

        let iter_def = StructDef {
            vis: quote! { pub },
            name: iter_name.clone(),
            generics: quote! { #iter_generics_tokens },
            fields: vec![StructField {
                vis: TokenStream::new(),
                name: syn::Ident::new("inner", proc_macro2::Span::call_site()),
                ty: inner_type,
            }],
        };

        let iter_impl_tokens = {
            let item = &matching_item;
            quote! {
                #[automatically_derived]
                impl #iter_impl_generics Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item;
                    fn next(&mut self) -> Option<Self::Item> {
                        self.inner.next()
                    }
                }
            }
        };

        Ok(vec![
            trait_impl.to_token_stream(),
            iter_def.to_token_stream(),
            iter_impl_tokens,
        ])
    }

    fn emit_enum(
        &self,
        ctx: &DeriveContext<'_, StandardLayout>,
        data: &ir::DataEnum<StandardLayout>,
    ) -> darling::Result<Vec<TokenStream>> {
        let full_trait_path = self.collection.full_trait_path(ctx);
        let iter_name = self.iter_type_name(ctx);
        let matching_item = self.collection.matching_item(ctx);
        let trait_generics = self.trait_generics();
        let needs_input_generics = data.variants.iter().any(|v| v.wraps.is_some());
        let iter_generics = self.iter_generics(ctx, needs_input_generics);
        let iter_generics_tokens = iter_generics.clone();
        let trait_lifetime = &self.collection.trait_lifetime;
        let trait_method = &self.collection.trait_method;
        let trait_type_iter = &self.collection.trait_type_iter;
        let input_name = &ctx.meta.name;

        let impl_generics = self.add_trait_lifetime(&ctx.meta.generics);
        let (iter_impl_generics, iter_ty_generics, iter_where_clause) =
            iter_generics.split_for_impl();

        let mut variant_idents = Vec::new();
        let mut variant_defs = Vec::new();
        let mut variant_exprs = Vec::new();

        for variant in &data.variants {
            let stmt_ctx = ctx
                .statements
                .get(&variant.name.to_string())
                .ok_or_else(|| {
                    darling::Error::custom(format!(
                        "missing statement context for '{}'",
                        variant.name
                    ))
                })?;

            let (iter_expr, inner_type) = self.collection.statement_iter(ctx, stmt_ctx);
            let variant_ident = &stmt_ctx.stmt.name;
            variant_idents.push(variant_ident.clone());

            variant_defs.push(EnumVariant {
                name: variant_ident.clone(),
                fields: vec![inner_type],
            });

            let pattern = &stmt_ctx.pattern;
            if stmt_ctx.pattern.is_empty() {
                variant_exprs.push(
                    quote! { Self::#variant_ident => #iter_name::#variant_ident(#iter_expr) },
                );
            } else {
                variant_exprs.push(
                    quote! { Self::#variant_ident #pattern => #iter_name::#variant_ident(#iter_expr) },
                );
            }
        }

        let self_arg = if self.collection.mutable {
            quote! { &#trait_lifetime mut self }
        } else {
            quote! { &#trait_lifetime self }
        };
        let body = if data.has_hidden_variants {
            quote! {
                match self {
                    #(#variant_exprs,)*
                    _ => unreachable!()
                }
            }
        } else {
            quote! {
                match self {
                    #(#variant_exprs),*
                }
            }
        };

        let (_, type_generics, _) = ctx.meta.generics.split_for_impl();
        let trait_impl = TraitImpl::new(impl_generics, &full_trait_path, input_name)
            .trait_generics(&trait_generics)
            .type_generics(type_generics)
            .assoc_type(
                trait_type_iter.clone(),
                quote! { #iter_name #iter_ty_generics },
            )
            .method(Method {
                name: trait_method.clone(),
                self_arg,
                params: vec![],
                return_type: Some(quote! { Self::#trait_type_iter }),
                body,
                generics: None,
                method_where_clause: None,
            });

        let iter_def = EnumDef {
            vis: quote! { pub },
            name: iter_name.clone(),
            generics: quote! { #iter_generics_tokens },
            variants: variant_defs,
        };

        let next_body = quote! {
            match self {
                #(Self::#variant_idents(inner) => inner.next(),)*
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

        Ok(vec![
            trait_impl.to_token_stream(),
            iter_def.to_token_stream(),
            iter_impl_tokens,
        ])
    }
}

impl Template<StandardLayout> for FieldIterTemplateSet {
    fn emit(&self, ctx: &DeriveContext<'_, StandardLayout>) -> darling::Result<Vec<TokenStream>> {
        match &ctx.input.data {
            ir::Data::Struct(data) => self.emit_struct(ctx, data),
            ir::Data::Enum(data) => self.emit_enum(ctx, data),
        }
    }
}
