use crate::ChumskyLayout;

/// Shared configuration for code generators.
#[derive(Clone)]
pub(crate) struct GeneratorConfig {
    /// Path to the kirin-chumsky crate (e.g., `::kirin::parsers`)
    pub crate_path: syn::Path,
    /// Path to the kirin IR crate (e.g., `::kirin::ir`)
    pub ir_path: syn::Path,
    /// The IR type path (e.g., `SimpleType`)
    pub ir_type: syn::Path,
}

impl GeneratorConfig {
    /// Creates a new generator config from IR input.
    pub fn new(ir_input: &kirin_derive_toolkit::ir::Input<ChumskyLayout>) -> Self {
        let crate_path = ir_input.extra_crate_path(|| syn::parse_quote!(::kirin::parsers));
        let ir_path = ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::ir));
        let ir_type = ir_input.attrs.ir_type.clone();
        Self {
            crate_path,
            ir_path,
            ir_type,
        }
    }
}
