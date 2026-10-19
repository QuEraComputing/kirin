use kirin_derive_toolkit::context::{DeriveContext, StatementContext};
use kirin_derive_toolkit::ir::StandardLayout;
use kirin_derive_toolkit::misc::from_str;
use kirin_derive_toolkit::prelude::darling;
use kirin_derive_toolkit::template::TraitImplTemplate;
use kirin_derive_toolkit::template::method_pattern::{Custom, MethodSpec};
use proc_macro2::TokenStream;
use quote::quote;

/// Body for one statement: forward through `#[wraps]`, otherwise no operands.
///
/// Structs bind their wrapped field with `let Self #pattern = self;`; enum
/// variants already have it bound by the generated `match`.
fn branch_op_body(
    stmt_ctx: &StatementContext<'_, StandardLayout>,
    full_trait_path: &syn::Path,
    trait_method: &syn::Ident,
    is_struct: bool,
) -> darling::Result<TokenStream> {
    if !stmt_ctx.is_wrapper {
        // A block, since enum variant bodies become `match` arm expressions.
        return Ok(quote! {{
            let _ = index;
            ::core::option::Option::None
        }});
    }
    let wrapper_ty = stmt_ctx
        .wrapper_type
        .expect("wrapper type should be present");
    let field = stmt_ctx
        .wrapper_binding
        .as_ref()
        .expect("wrapper binding should be present");
    let call = quote! { <#wrapper_ty as #full_trait_path>::#trait_method(#field, index) };
    if is_struct {
        let pattern = &stmt_ctx.pattern;
        Ok(quote! {
            let Self #pattern = self;
            #call
        })
    } else {
        Ok(call)
    }
}

fn branch_op_method(
    crate_path: &syn::Path,
    name: &str,
    self_arg: TokenStream,
    return_type: TokenStream,
) -> MethodSpec<StandardLayout> {
    let mut full_trait_path = crate_path.clone();
    full_trait_path
        .segments
        .push(syn::PathSegment::from(syn::Ident::new(
            "BranchOp",
            proc_macro2::Span::call_site(),
        )));
    let trait_method: syn::Ident = from_str(name);
    let (struct_path, struct_method) = (full_trait_path.clone(), trait_method.clone());
    let pattern = Custom::separate(
        move |_ctx: &DeriveContext<'_, StandardLayout>, stmt_ctx| {
            branch_op_body(stmt_ctx, &struct_path, &struct_method, true)
        },
        move |_ctx: &DeriveContext<'_, StandardLayout>, stmt_ctx| {
            branch_op_body(stmt_ctx, &full_trait_path, &trait_method, false)
        },
    );
    MethodSpec {
        name: from_str(name),
        self_arg,
        params: vec![quote! { index: usize }],
        return_type: Some(return_type),
        pattern: Box::new(pattern),
        generics: None,
        method_where_clause: None,
    }
}

/// Create a `TraitImplTemplate` that generates `impl BranchOp for Type`.
///
/// Wrapper statements forward to the wrapped type; every other statement
/// forwards no operands. Dialects marked `#[kirin(branch_op)]` implement the
/// trait by hand, and the caller skips this template for them.
pub(crate) fn branch_op_template(crate_path: &syn::Path) -> TraitImplTemplate<StandardLayout> {
    let trait_path: syn::Path = from_str("BranchOp");
    let default_crate: syn::Path = from_str("::kirin::ir");
    TraitImplTemplate::new(trait_path, default_crate)
        .method(branch_op_method(
            crate_path,
            "successor_operands",
            quote! { &self },
            quote! { ::core::option::Option<&[#crate_path::SSAValue]> },
        ))
        .method(branch_op_method(
            crate_path,
            "successor_operands_mut",
            quote! { &mut self },
            quote! { ::core::option::Option<&mut ::std::vec::Vec<#crate_path::SSAValue>> },
        ))
}
