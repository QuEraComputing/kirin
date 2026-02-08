use kirin_derive_core::prelude::*;
use std::collections::HashMap;

pub(crate) fn emit_from_derive_input<T>(
    derive: &mut T,
    input: &syn::DeriveInput,
) -> darling::Result<proc_macro2::TokenStream>
where
    for<'ir> T: Scan<'ir, StandardLayout> + Emit<'ir, StandardLayout>,
{
    let input = ir::Input::<StandardLayout>::from_derive_input(input)?;
    derive.scan_input(&input)?;
    derive.emit_input(&input)
}

pub(crate) fn require_input_ctx<'a, T>(
    input: &'a Option<T>,
    derive_name: &str,
) -> darling::Result<&'a T> {
    input.as_ref().ok_or_else(|| {
        darling::Error::custom(format!(
            "{derive_name} context missing, call scan_input first"
        ))
    })
}

pub(crate) fn statement_info<'a, T>(
    statements: &'a HashMap<String, T>,
    statement: &ir::Statement<StandardLayout>,
) -> darling::Result<&'a T> {
    let key = statement.name.to_string();
    statements.get(&key).ok_or_else(|| {
        darling::Error::custom(format!(
            "Missing statement info for '{}', call scan_statement first",
            key
        ))
    })
}
