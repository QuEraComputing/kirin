use kirin_derive_toolkit::context::StatementContext;
use kirin_derive_toolkit::ir::StandardLayout;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

/// `DeclaredName` follows `HasCallableBody`: an unmarked statement returns
/// `None`, a `#[kirin(declared_name)]` field returns that symbol, and
/// `#[wraps]` delegates.
pub(crate) fn declared_name_template(crate_path: &syn::Path) -> TraitImplTemplate<StandardLayout> {
    let struct_crate = crate_path.clone();
    let variant_crate = crate_path.clone();
    let bounds_crate = crate_path.clone();
    TraitImplTemplate::new(from_str("DeclaredName"), from_str("::kirin::ir"))
        .where_clause(move |ctx| {
            let bounds: Vec<syn::WherePredicate> = ctx
                .statements
                .values()
                .filter_map(|stmt| stmt.wrapper_type)
                .map(|ty| syn::parse_quote!(#ty: #bounds_crate::DeclaredName))
                .collect();
            (!bounds.is_empty()).then(|| syn::parse_quote!(where #(#bounds),*))
        })
        .method(MethodSpec {
            name: from_str("declared_name"),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some(quote! { ::core::option::Option<#crate_path::Symbol> }),
            pattern: Box::new(Custom::separate(
                move |_, stmt| {
                    let name = projection(stmt, &struct_crate);
                    if !stmt.is_wrapper && stmt.stmt.declared_name.is_none() {
                        return Ok(name);
                    }
                    let pattern = &stmt.pattern;
                    Ok(quote! { let Self #pattern = self; #name })
                },
                move |_, stmt| Ok(projection(stmt, &variant_crate)),
            )),
            generics: None,
            method_where_clause: None,
        })
}

fn projection(stmt: &StatementContext<'_, StandardLayout>, ir: &syn::Path) -> TokenStream {
    if let (Some(ty), Some(binding)) = (stmt.wrapper_type, &stmt.wrapper_binding) {
        return quote! { <#ty as #ir::DeclaredName>::declared_name(#binding) };
    }
    match &stmt.stmt.declared_name {
        Some(field) => {
            let binding = field.name();
            quote! { ::core::option::Option::Some(*#binding) }
        }
        None => quote! { ::core::option::Option::None },
    }
}
