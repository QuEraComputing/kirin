//! Code generation for the `PrettyPrint` derive macro.
//!
//! This generates `PrettyPrint` implementations for dialect types based on their
//! `chumsky(format = "...")` attributes. The generated printer mirrors the parser,
//! ensuring roundtrip compatibility.

use proc_macro2::TokenStream;

use crate::PrettyPrintLayout;

/// Generator for the `PrettyPrint` trait implementation.
pub struct GeneratePrettyPrint {
    /// Path to the kirin_prettyless crate
    pub(super) prettyless_path: syn::Path,
}

impl GeneratePrettyPrint {
    /// Creates a new generator.
    ///
    /// Resolution order for the prettyless crate path:
    /// 1. `#[pretty(crate = ...)]` -- explicit prettyless path
    /// 2. `#[kirin(crate = ...)]` -- derive from kirin crate path
    /// 3. Default: `::kirin::pretty`
    pub fn new(ir_input: &kirin_derive_core::ir::Input<PrettyPrintLayout>) -> Self {
        // If #[pretty(crate = ...)] is specified, use it directly
        if let Some(path) = &ir_input.extra_attrs.crate_path {
            return Self {
                prettyless_path: path.clone(),
            };
        }

        // Fall back to deriving from #[kirin(crate = ...)]
        let prettyless_path = ir_input
            .attrs
            .crate_path
            .as_ref()
            .map(|p| {
                // If user specified a kirin crate re-export path, derive prettyless as sibling
                // e.g., `kirin::parsers` -> `kirin::pretty`
                let mut segments = p.segments.clone();
                if let Some(last) = segments.last_mut() {
                    if last.ident == "parsers" {
                        last.ident = syn::Ident::new("pretty", last.ident.span());
                        return syn::Path {
                            leading_colon: p.leading_colon,
                            segments,
                        };
                    }
                }
                // Otherwise fall back to default
                syn::parse_quote!(::kirin::pretty)
            })
            .unwrap_or_else(|| syn::parse_quote!(::kirin::pretty));
        Self { prettyless_path }
    }

    /// Generates the `PrettyPrint` implementation.
    ///
    /// Generates `impl PrettyPrint for Self` with a method generic over `L: Dialect`.
    /// This avoids the trait resolution overflow that occurred with the old
    /// `impl<L: Dialect + PrettyPrint<L>> PrettyPrint<L> for Block` pattern.
    pub fn generate(
        &self,
        ir_input: &kirin_derive_core::ir::Input<PrettyPrintLayout>,
    ) -> TokenStream {
        // For wrapper structs, forward to the wrapped type's PrettyPrint
        if let kirin_derive_core::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self.generate_wrapper_struct_pretty_print(ir_input, wrapper);
            }
        }

        self.generate_pretty_print(ir_input)
    }

    /// Resolves the IR crate path from `#[kirin(crate = ...)]` or default.
    pub(super) fn ir_path(ir_input: &kirin_derive_core::ir::Input<PrettyPrintLayout>) -> syn::Path {
        ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::ir))
    }
}
