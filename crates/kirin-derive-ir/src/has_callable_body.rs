use kirin_derive_toolkit::context::StatementContext;
use kirin_derive_toolkit::ir::StandardLayout;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

/// Generate a structural projection using the same template machinery as
/// HasSignature, without sharing its dialect/type parameter.
pub(crate) fn has_callable_body_template(
    crate_path: &syn::Path,
) -> TraitImplTemplate<StandardLayout> {
    let struct_crate = crate_path.clone();
    let variant_crate = crate_path.clone();
    let bounds_crate = crate_path.clone();
    TraitImplTemplate::new(from_str("HasCallableBody"), from_str("::kirin::ir"))
        .where_clause(move |ctx| {
            let bounds: Vec<syn::WherePredicate> = ctx
                .statements
                .values()
                .filter_map(|stmt| stmt.wrapper_type)
                .map(|ty| syn::parse_quote!(#ty: #bounds_crate::HasCallableBody))
                .collect();
            (!bounds.is_empty()).then(|| syn::parse_quote!(where #(#bounds),*))
        })
        .method(MethodSpec {
            name: from_str("callable_body"),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some(quote! { ::core::option::Option<#crate_path::Body> }),
            pattern: Box::new(Custom::separate(
                move |_, stmt| {
                    let body = projection(stmt, &struct_crate);
                    if !stmt.is_wrapper && stmt.stmt.callable_body.is_none() {
                        return Ok(body);
                    }
                    let pattern = &stmt.pattern;
                    // Use the same bindings as enum arms, including tuple fields.
                    Ok(quote! { let Self #pattern = self; #body })
                },
                move |_, stmt| Ok(projection(stmt, &variant_crate)),
            )),
            generics: None,
            method_where_clause: None,
        })
}

fn projection(stmt: &StatementContext<'_, StandardLayout>, ir: &syn::Path) -> TokenStream {
    if let (Some(ty), Some(binding)) = (stmt.wrapper_type, &stmt.wrapper_binding) {
        return quote! { <#ty as #ir::HasCallableBody>::callable_body(#binding) };
    }
    match &stmt.stmt.callable_body {
        Some(field) => {
            let binding = field.name();
            quote! { ::core::option::Option::Some(#ir::Body::from(*#binding)) }
        }
        None => quote! { ::core::option::Option::None },
    }
}
