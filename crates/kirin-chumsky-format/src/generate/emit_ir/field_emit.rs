use kirin_derive_core::ir::fields::FieldInfo;
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use crate::field_kind::FieldKind;

use super::GenerateEmitIR;

impl GenerateEmitIR {
    pub(super) fn generate_field_emit_calls(
        &self,
        ast_fields: &[&FieldInfo<ChumskyLayout>],
        field_vars: &[syn::Ident],
        generics: &syn::Generics,
        _is_tuple: bool,
    ) -> TokenStream {
        let crate_path = &self.config.crate_path;

        // Get type parameter names for checking if a Value type needs EmitIR::emit
        let type_param_names: Vec<String> = generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect();

        // ast_fields and field_vars should already be in the correct order
        let emit_stmts: Vec<_> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(field, var)| {
                let emitted_var =
                    syn::Ident::new(&format!("{}_ir", var), proc_macro2::Span::call_site());

                // Use FieldKind to determine the emit behavior
                let kind = FieldKind::from_field_info(field);
                match kind {
                    FieldKind::SSAValue
                    | FieldKind::ResultValue
                    | FieldKind::Block
                    | FieldKind::Successor
                    | FieldKind::Region
                    | FieldKind::Symbol => quote! {
                        let #emitted_var = #crate_path::EmitIR::emit(#var, ctx);
                    },
                    FieldKind::Value(ref ty) => {
                        // Check if this Value type contains any type parameters
                        let needs_emit_ir = type_param_names.iter().any(|param_name| {
                            kirin_derive_core::misc::is_type(ty, param_name.as_str())
                                || kirin_derive_core::misc::is_type_in_generic(
                                    ty,
                                    param_name.as_str(),
                                )
                        });

                        if needs_emit_ir {
                            // For Value types containing type parameters, call EmitIR::emit
                            // to convert from the AST representation to the IR representation
                            quote! {
                                let #emitted_var = #crate_path::EmitIR::emit(#var, ctx);
                            }
                        } else {
                            // For concrete Value types, just clone directly
                            // (the AST type equals the IR type via HasParser<Output = T>)
                            quote! {
                                let #emitted_var = #var.clone();
                            }
                        }
                    }
                }
            })
            .collect();

        quote! {
            #(#emit_stmts)*
        }
    }
}
