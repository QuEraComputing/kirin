use kirin_derive_toolkit::ir::fields::{FieldCategory, FieldInfo};
use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

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

        let type_param_names: Vec<String> = generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect();

        let emit_stmts: Vec<_> = ast_fields
            .iter()
            .zip(field_vars.iter())
            .map(|(field, var)| {
                let emitted_var =
                    syn::Ident::new(&format!("{}_ir", var), proc_macro2::Span::call_site());

                match field.category() {
                    FieldCategory::Value => {
                        let ty = field
                            .value_type()
                            .cloned()
                            .unwrap_or_else(|| syn::parse_quote!(()));
                        let needs_emit_ir = type_param_names.iter().any(|param_name| {
                            kirin_derive_toolkit::misc::is_type(&ty, param_name.as_str())
                                || kirin_derive_toolkit::misc::is_type_in_generic(
                                    &ty,
                                    param_name.as_str(),
                                )
                        });

                        if needs_emit_ir {
                            quote! {
                                let #emitted_var = #crate_path::EmitIR::emit(#var, ctx)?;
                            }
                        } else {
                            quote! {
                                let #emitted_var = #var.clone();
                            }
                        }
                    }
                    _ => quote! {
                        let #emitted_var = #crate_path::EmitIR::emit(#var, ctx)?;
                    },
                }
            })
            .collect();

        quote! {
            #(#emit_stmts)*
        }
    }
}
