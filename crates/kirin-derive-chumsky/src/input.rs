//! Derive input parsing for chumsky macros.

use kirin_derive_toolkit::ir::fields::FieldCategory;

use crate::{ChumskyLayout, PrettyPrintLayout};

/// Parses derive input for chumsky macros.
///
/// For value-only definitions, `#[kirin(type = ...)]` is optional.
/// For dialect-like definitions using SSA/Result/Block/Region fields,
/// `#[kirin(type = ...)]` remains required.
pub fn parse_derive_input(
    ast: &syn::DeriveInput,
) -> darling::Result<kirin_derive_toolkit::ir::Input<ChumskyLayout>> {
    match kirin_derive_toolkit::ir::Input::<ChumskyLayout>::from_derive_input(ast) {
        Ok(input) => return Ok(input),
        Err(err) if !is_missing_type_error(&err) => return Err(err),
        Err(_) => {}
    }

    // Value-only definitions can omit #[kirin(type = ...)].
    // Inject a placeholder type so shared IR input parsing can proceed.
    let mut patched = ast.clone();
    patched.attrs.push(syn::parse_quote!(#[kirin(type = bool)]));
    let input = kirin_derive_toolkit::ir::Input::<ChumskyLayout>::from_derive_input(&patched)?;

    if input_requires_ir_type(&input) {
        return Err(darling::Error::custom(
            "`#[kirin(type = ...)]` is required when using SSAValue, ResultValue, Block, or Region fields",
        )
        .with_span(&ast.ident));
    }

    Ok(input)
}

fn is_missing_type_error(err: &darling::Error) -> bool {
    let message = err.to_string();
    message.contains("Missing field `type`") || message.contains("missing field `type`")
}

fn input_requires_ir_type<L: kirin_derive_toolkit::ir::Layout>(
    input: &kirin_derive_toolkit::ir::Input<L>,
) -> bool {
    match &input.data {
        kirin_derive_toolkit::ir::Data::Struct(data) => statement_requires_ir_type(&data.0),
        kirin_derive_toolkit::ir::Data::Enum(data) => {
            data.variants.iter().any(statement_requires_ir_type)
        }
    }
}

fn statement_requires_ir_type<L: kirin_derive_toolkit::ir::Layout>(
    stmt: &kirin_derive_toolkit::ir::Statement<L>,
) -> bool {
    stmt.collect_fields().iter().any(|field| {
        matches!(
            field.category(),
            FieldCategory::Argument
                | FieldCategory::Result
                | FieldCategory::Block
                | FieldCategory::Region
        )
    })
}

/// Parses derive input for the `PrettyPrint` derive macro.
///
/// Same as [`parse_derive_input`] but uses [`PrettyPrintLayout`] to parse
/// `#[pretty(crate = ...)]` as the global attrs.
pub fn parse_pretty_derive_input(
    ast: &syn::DeriveInput,
) -> darling::Result<kirin_derive_toolkit::ir::Input<PrettyPrintLayout>> {
    match kirin_derive_toolkit::ir::Input::<PrettyPrintLayout>::from_derive_input(ast) {
        Ok(input) => return Ok(input),
        Err(err) if !is_missing_type_error(&err) => return Err(err),
        Err(_) => {}
    }

    let mut patched = ast.clone();
    patched.attrs.push(syn::parse_quote!(#[kirin(type = bool)]));
    let input = kirin_derive_toolkit::ir::Input::<PrettyPrintLayout>::from_derive_input(&patched)?;

    if input_requires_ir_type(&input) {
        return Err(darling::Error::custom(
            "`#[kirin(type = ...)]` is required when using SSAValue, ResultValue, Block, or Region fields",
        )
        .with_span(&ast.ident));
    }

    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_input_without_type_annotation() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[chumsky(format = "$literal {value}")]
            struct Literal {
                value: i64,
            }
        };
        let result = parse_derive_input(&input);
        assert!(
            result.is_ok(),
            "Value-only struct should not require #[kirin(type)]"
        );
    }

    #[test]
    fn test_parse_input_ssa_without_type_requires_annotation() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[chumsky(format = "$add {lhs}, {rhs} -> {result:type}")]
            struct Add {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        let result = parse_derive_input(&input);
        assert!(result.is_err(), "SSA fields should require #[kirin(type)]");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("kirin(type"),
            "Error should mention kirin(type): {err}"
        );
    }

    #[test]
    fn test_parse_input_with_type_annotation() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type = SimpleType)]
            #[chumsky(format = "$add {lhs}, {rhs} -> {result:type}")]
            struct Add {
                result: SSAValue,
                lhs: Value,
                rhs: Value,
            }
        };
        let result = parse_derive_input(&input);
        assert!(
            result.is_ok(),
            "Should parse with type annotation: {:?}",
            result.err()
        );
    }
}
