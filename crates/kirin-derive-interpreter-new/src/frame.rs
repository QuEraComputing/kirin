use proc_macro2::TokenStream;
use quote::quote;

const DEFAULT_INTERP_CRATE: &str = "::kirin_interpreter_new";

pub fn do_derive_has_location(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let interp_crate = parse_interpret_crate_path(input)?;
    let variants = wrapper_variants(input)?;
    let type_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let binding = &variant.binding;
        let pattern = &variant.pattern;
        quote! { Self::#name #pattern => #binding.location() }
    });

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::HasLocation for #type_name #ty_generics #where_clause {
            fn location(&self) -> #interp_crate::Location {
                match self {
                    #(#arms),*
                }
            }
        }
    })
}

pub fn do_derive_frame(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let interp_crate = parse_interpret_crate_path(input)?;
    let variants = wrapper_variants(input)?;
    let type_name = &input.ident;
    let mut generics = input.generics.clone();
    generics
        .params
        .insert(0, syn::GenericParam::Type(syn::parse_quote!(__FrameI)));
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__FrameF)));
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__FrameC)));
    generics
        .params
        .push(syn::GenericParam::Type(syn::parse_quote!(__FrameE)));
    let (impl_generics, _, _) = generics.split_for_impl();
    let (_, ty_generics, original_where) = input.generics.split_for_impl();

    let mut predicates: Vec<syn::WherePredicate> = Vec::new();
    for variant in &variants {
        let ty = &variant.ty;
        predicates.push(syn::parse_quote! {
            #ty: #interp_crate::Frame<__FrameI, __FrameF, __FrameC, __FrameE>
        });
    }
    let extra_where: Option<syn::WhereClause> = if predicates.is_empty() {
        None
    } else {
        Some(syn::parse_quote! { where #(#predicates),* })
    };
    let where_clause =
        kirin_derive_toolkit::codegen::combine_where_clauses(extra_where.as_ref(), original_where);

    let step_arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let binding = &variant.binding;
        let pattern = &variant.owned_pattern;
        quote! { Self::#name #pattern => #binding.step(interp) }
    });
    let resume_done_arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let binding = &variant.binding;
        let pattern = &variant.owned_pattern;
        quote! { Self::#name #pattern => #binding.resume_done(interp) }
    });
    let resume_arms = variants.iter().map(|variant| {
        let name = &variant.ident;
        let binding = &variant.binding;
        let pattern = &variant.owned_pattern;
        quote! { Self::#name #pattern => #binding.resume(completion, interp) }
    });

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #interp_crate::Frame<__FrameI, __FrameF, __FrameC, __FrameE>
            for #type_name #ty_generics
        #where_clause
        {
            fn step(
                self,
                interp: &mut __FrameI,
            ) -> Result<#interp_crate::FrameEffect<__FrameF, __FrameC>, __FrameE> {
                match self {
                    #(#step_arms),*
                }
            }

            fn resume_done(
                self,
                interp: &mut __FrameI,
            ) -> Result<#interp_crate::FrameEffect<__FrameF, __FrameC>, __FrameE> {
                match self {
                    #(#resume_done_arms),*
                }
            }

            fn resume(
                self,
                completion: __FrameC,
                interp: &mut __FrameI,
            ) -> Result<#interp_crate::FrameEffect<__FrameF, __FrameC>, __FrameE> {
                match self {
                    #(#resume_arms),*
                }
            }
        }
    })
}

struct WrapperVariant {
    ident: syn::Ident,
    ty: syn::Type,
    binding: syn::Ident,
    pattern: TokenStream,
    owned_pattern: TokenStream,
}

fn wrapper_variants(input: &syn::DeriveInput) -> syn::Result<Vec<WrapperVariant>> {
    let syn::Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "derive only supports single-field enum variants",
        ));
    };

    data.variants
        .iter()
        .map(|variant| {
            let binding = syn::Ident::new("__frame", variant.ident.span());
            match &variant.fields {
                syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let ty = fields.unnamed[0].ty.clone();
                    Ok(WrapperVariant {
                        ident: variant.ident.clone(),
                        ty,
                        binding: binding.clone(),
                        pattern: quote! { (#binding) },
                        owned_pattern: quote! { (#binding) },
                    })
                }
                syn::Fields::Named(fields) if fields.named.len() == 1 => {
                    let field = fields.named.iter().next().unwrap();
                    let ident = field.ident.clone().unwrap();
                    let ty = field.ty.clone();
                    Ok(WrapperVariant {
                        ident: variant.ident.clone(),
                        ty,
                        binding: binding.clone(),
                        pattern: quote! { { #ident: #binding } },
                        owned_pattern: quote! { { #ident: #binding } },
                    })
                }
                _ => Err(syn::Error::new_spanned(
                    variant,
                    "derive only supports variants with exactly one field",
                )),
            }
        })
        .collect()
}

fn parse_interpret_crate_path(input: &syn::DeriveInput) -> syn::Result<syn::Path> {
    let mut crate_path = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("interpret") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let value = meta.value()?;
                crate_path = Some(value.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute for #[interpret(...)]"))
            }
        })?;
    }
    Ok(crate_path.unwrap_or_else(|| syn::parse_str(DEFAULT_INTERP_CRATE).unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_test_utils::rustfmt;

    fn generate_has_location_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_has_location(&input).expect("failed to generate HasLocation");
        rustfmt(tokens.to_string())
    }

    fn generate_frame_code(input: syn::DeriveInput) -> String {
        let tokens = do_derive_frame(&input).expect("failed to generate Frame");
        rustfmt(tokens.to_string())
    }

    #[test]
    fn has_location_for_frame_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
                Standard(StandardFrame<L, V, T>),
                Scf(ScfFrame<L, ArithType, V, T>),
            }
        };
        insta::assert_snapshot!(generate_has_location_code(input));
    }

    #[test]
    fn frame_for_frame_enum() {
        let input: syn::DeriveInput = syn::parse_quote! {
            enum ToyFrame<L: Dialect, V, T = ConcreteBlockTransfer<V>> {
                Standard(StandardFrame<L, V, T>),
                Scf(ScfFrame<L, ArithType, V, T>),
            }
        };
        insta::assert_snapshot!(generate_frame_code(input));
    }
}
