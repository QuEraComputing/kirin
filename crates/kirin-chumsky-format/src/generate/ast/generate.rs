//! Code generation for AST types corresponding to dialect definitions.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ChumskyLayout;

use kirin_derive_core::codegen::deduplicate_types;

use crate::generate::{
    GeneratorConfig, collect_all_value_types_needing_bounds,
};

/// Generator for AST type definitions.
///
/// This generates the AST type (e.g., `MyDialectAST`) that corresponds to a dialect
/// definition. The AST type is used during parsing to represent the syntax tree
/// before it's converted to IR.
///
/// AST types are parameterized by `TypeOutput` and `LanguageOutput` instead of `Language: Dialect`.
/// This avoids GAT projection issues that cause infinite compilation times.
pub struct GenerateAST {
    pub(super) config: GeneratorConfig,
}

impl GenerateAST {
    /// Creates a new generator.
    pub fn new(ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> Self {
        Self {
            config: GeneratorConfig::new(ir_input),
        }
    }

    /// Generates the AST type definition with derive(Clone, Debug, PartialEq).
    ///
    /// For wrapper structs, no AST type is generated - the HasParser/HasDialectParser
    /// impls forward directly to the wrapped type's impls.
    ///
    /// This also generates the `ASTSelf` wrapper type for standalone use.
    pub fn generate(&self, ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> TokenStream {
        // For wrapper structs, don't generate any AST type.
        // The HasParser/HasDialectParser impls will forward to the wrapped type.
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if data.0.wraps.is_some() {
                return TokenStream::new();
            }
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
    ///
    /// This includes:
    /// - Value field types that contain type parameters
    /// - ir_type if it contains type parameters
    pub(super) fn collect_value_types_needing_bounds(
        &self,
        ir_input: &kirin_derive_core::ir::Input<ChumskyLayout>,
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
            if kirin_derive_core::misc::is_type(&ir_type_ty, param_name.as_str())
                || kirin_derive_core::misc::is_type_in_generic(&ir_type_ty, param_name.as_str())
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
