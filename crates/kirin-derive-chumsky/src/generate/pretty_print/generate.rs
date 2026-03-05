use proc_macro2::TokenStream;

use crate::PrettyPrintLayout;

pub struct GeneratePrettyPrint {
    pub(super) prettyless_path: syn::Path,
}

impl GeneratePrettyPrint {
    pub fn new(ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>) -> Self {
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
                syn::parse_quote!(::kirin::pretty)
            })
            .unwrap_or_else(|| syn::parse_quote!(::kirin::pretty));
        Self { prettyless_path }
    }

    pub fn generate(
        &self,
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
    ) -> TokenStream {
        if let kirin_derive_toolkit::ir::Data::Struct(data) = &ir_input.data {
            if let Some(wrapper) = &data.0.wraps {
                return self.generate_wrapper_struct_pretty_print(ir_input, wrapper);
            }
        }

        self.generate_pretty_print(ir_input)
    }

    pub(super) fn ir_path(
        ir_input: &kirin_derive_toolkit::ir::Input<PrettyPrintLayout>,
    ) -> syn::Path {
        ir_input
            .attrs
            .crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_quote!(::kirin::ir))
    }
}
