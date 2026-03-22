use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::StandardLayout;
use kirin_derive_toolkit::ir::fields::FieldCategory;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

/// Build the body for a struct's HasSignature method.
fn signature_body_struct(
    _ctx: &DeriveContext<'_, StandardLayout>,
    stmt_ctx: &StatementContext<'_, StandardLayout>,
    full_trait_path: &syn::Path,
    trait_method: &syn::Ident,
) -> darling::Result<TokenStream> {
    if stmt_ctx.is_wrapper {
        let wrapper_ty = stmt_ctx
            .wrapper_type
            .expect("wrapper type should be present");
        let field = stmt_ctx
            .wrapper_binding
            .as_ref()
            .expect("wrapper binding should be present");
        let pattern = &stmt_ctx.pattern;
        return Ok(quote! {
            let Self #pattern = self;
            <#wrapper_ty as #full_trait_path<#wrapper_ty>>::#trait_method(#field)
        });
    }

    let sig_field = stmt_ctx
        .stmt
        .iter_all_fields()
        .find(|f| f.category() == FieldCategory::Signature);

    match sig_field {
        Some(field) => {
            // For structs, use `self.field` since the method receives &self.
            let field_ident = field.ident.as_ref().expect("Signature field must be named");
            Ok(quote! { Some(self.#field_ident.clone()) })
        }
        None => Ok(quote! { None }),
    }
}

/// Build the body for an enum variant's HasSignature match arm.
fn signature_body_variant(
    _ctx: &DeriveContext<'_, StandardLayout>,
    stmt_ctx: &StatementContext<'_, StandardLayout>,
    full_trait_path: &syn::Path,
    trait_method: &syn::Ident,
) -> darling::Result<TokenStream> {
    if stmt_ctx.is_wrapper {
        let wrapper_ty = stmt_ctx
            .wrapper_type
            .expect("wrapper type should be present");
        let field = stmt_ctx
            .wrapper_binding
            .as_ref()
            .expect("wrapper binding should be present");
        return Ok(
            quote! { <#wrapper_ty as #full_trait_path<#wrapper_ty>>::#trait_method(#field) },
        );
    }

    let sig_field = stmt_ctx
        .stmt
        .iter_all_fields()
        .find(|f| f.category() == FieldCategory::Signature);

    match sig_field {
        Some(field) => {
            // For enum variants, use the pattern binding variable from destructuring.
            let binding = &stmt_ctx.pattern.names[field.index];
            Ok(quote! { Some(#binding.clone()) })
        }
        None => Ok(quote! { None }),
    }
}

/// Create a `TraitImplTemplate` that generates `impl HasSignature<Self> for Type`.
pub(crate) fn has_signature_template(crate_path: &syn::Path) -> TraitImplTemplate<StandardLayout> {
    let trait_path: syn::Path = from_str("HasSignature");
    let default_crate: syn::Path = from_str("::kirin::ir");

    let full_trait_for_closures = {
        let mut p = crate_path.clone();
        p.segments.push(syn::PathSegment::from(syn::Ident::new(
            "HasSignature",
            proc_macro2::Span::call_site(),
        )));
        p
    };
    let trait_method: syn::Ident = from_str("signature");
    let cp1 = full_trait_for_closures.clone();
    let m1 = trait_method.clone();
    let cp2 = full_trait_for_closures;
    let m2 = trait_method;

    let pattern = Custom::separate(
        move |ctx, stmt_ctx| signature_body_struct(ctx, stmt_ctx, &cp1, &m1),
        move |ctx, stmt_ctx| signature_body_variant(ctx, stmt_ctx, &cp2, &m2),
    );

    let cp_for_return = crate_path.clone();

    TraitImplTemplate::new(trait_path, default_crate)
        .trait_generics(|_ctx| quote! { <Self> })
        .method(MethodSpec {
            name: from_str("signature"),
            self_arg: quote! { &self },
            params: vec![],
            return_type: Some({
                quote! { Option<#cp_for_return::Signature<<Self as #cp_for_return::Dialect>::Type>> }
            }),
            pattern: Box::new(pattern),
            generics: None,
            method_where_clause: None,
        })
}
