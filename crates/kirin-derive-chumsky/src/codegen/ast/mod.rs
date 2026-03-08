//! Code generation for AST types corresponding to dialect definitions.

mod definition;
mod trait_impls;
mod wrapper;

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use kirin_derive_toolkit::codegen::deduplicate_types;

use crate::codegen::{GeneratorConfig, collect_all_value_types_needing_bounds};

/// Generator for AST type definitions.
pub struct GenerateAST {
    pub(in crate::codegen) config: GeneratorConfig,
}

impl GenerateAST {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the AST type definition with derive(Clone, Debug, PartialEq).
    pub fn generate(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> TokenStream {
        // For wrapper structs, don't generate any AST type.
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data
            && data.0.wraps.is_some()
        {
            return TokenStream::new();
        }

        let ast_name = syn::Ident::new(&format!("{}AST", ir_input.name), ir_input.name.span());

        let ast_def = self.generate_ast_definition(ir_input, &ast_name);
        let ast_self = self.generate_ast_self_wrapper(ir_input, &ast_name);

        quote! {
            #ast_def
            #ast_self
        }
    }

    /// Collects all types that contain type parameters and need HasParser bounds.
    pub(in crate::codegen) fn collect_value_types_needing_bounds(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>,
    ) -> Vec<syn::Type> {
        let mut all_types = Vec::new();

        // Collect type parameter names
        let type_param_names: Vec<String> = ir_input
            .generics
            .type_params()
            .map(|p| p.ident.to_string())
            .collect();

        // Check if ir_type contains any type parameter
        let ir_type = &self.config.ir_type;
        let ir_type_ty: syn::Type = syn::parse_quote!(#ir_type);
        for param_name in &type_param_names {
            if kirin_derive_toolkit::misc::is_type(&ir_type_ty, param_name.as_str())
                || kirin_derive_toolkit::misc::is_type_in_generic(&ir_type_ty, param_name.as_str())
            {
                all_types.push(ir_type_ty.clone());
                break;
            }
        }

        // Collect value field types from all statements
        all_types.extend(collect_all_value_types_needing_bounds(ir_input));
        deduplicate_types(&mut all_types);

        all_types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::parse_derive_input;
    use kirin_test_utils::rustfmt;

    fn generate_ast_code(input: syn::DeriveInput) -> String {
        let ir_input = parse_derive_input(&input).expect("Failed to parse derive input");
        let generator = GenerateAST::new(&ir_input);
        let tokens = generator.generate(&ir_input);
        rustfmt(tokens.to_string())
    }

    #[test]
    fn test_ast_struct_definition() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(crate = kirin_chumsky, format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
            struct Add {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        insta::assert_snapshot!(generate_ast_code(input));
    }

    #[test]
    fn test_ast_enum_definition() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum ArithOps {
                #[chumsky(format = "{result:name} = {.add} {lhs}, {rhs} -> {result:type}")]
                Add {
                    result: SSAValue,
                    lhs: Value,
                    rhs: Value,
                },
                #[chumsky(format = "{result:name} = {.neg} {operand} -> {result:type}")]
                Neg {
                    result: SSAValue,
                    operand: Value,
                },
            }
        };
        insta::assert_snapshot!(generate_ast_code(input));
    }

    #[test]
    fn test_ast_wrapper_enum_definition() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            enum MyLanguage {
                #[wraps]
                #[chumsky(format = "arith")]
                Arith(ArithOps),
                #[wraps]
                Cf(CfOps),
            }
        };
        insta::assert_snapshot!(generate_ast_code(input));
    }
}
