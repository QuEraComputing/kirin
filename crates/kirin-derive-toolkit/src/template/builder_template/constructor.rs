use quote::quote;

use super::helpers::{self, StatementInfo};
use crate::codegen::ConstructorBuilder;
use crate::context::InputMeta;
use crate::ir::StandardLayout;
use crate::ir::fields::FieldInfo;

/// The `new_{op}` constructor emitted next to a statement's builder.
///
/// It takes every field without a `#[kirin(default)]`, results included, in
/// declaration order, and returns the statement as a value. Unlike the
/// builder, it neither touches a stage nor creates the results, so the
/// caller can put the statement in any language that wraps it.
pub(super) fn constructor_fn(info: &StatementInfo, input: &InputMeta) -> proc_macro2::TokenStream {
    let name = &info.constructor_name;
    let params = info
        .fields
        .iter()
        .filter_map(|field| helpers::field_param(field, &input.ir_type));
    let let_inputs = helpers::build_fn_let_inputs(info, &input.ir_type);

    let is_tuple = info.fields.iter().all(|f| f.ident.is_none());
    let field_value = |field: &FieldInfo<StandardLayout>| {
        let name = field.name_ident(info.name.span());
        quote! { #name }
    };
    let constructor = if input.is_enum {
        ConstructorBuilder::new_variant(&input.name, &info.name, is_tuple)
            .build_with_self(&info.fields, field_value)
    } else {
        ConstructorBuilder::new_struct(&input.name, is_tuple)
            .build_with_self(&info.fields, field_value)
    };

    quote! {
        #[allow(clippy::too_many_arguments)]
        pub fn #name(#(#params),*) -> Self {
            #(#let_inputs)*
            #constructor
        }
    }
}
