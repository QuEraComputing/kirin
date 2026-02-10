//! Derive input parsing for chumsky macros.

use kirin_derive_core::ir::fields::FieldCategory;

use crate::ChumskyLayout;

/// Parses derive input for chumsky macros.
///
/// For value-only definitions, `#[kirin(type = ...)]` is optional.
/// For dialect-like definitions using SSA/Result/Block/Region fields,
/// `#[kirin(type = ...)]` remains required.
pub fn parse_derive_input(
    ast: &syn::DeriveInput,
) -> darling::Result<kirin_derive_core::ir::Input<ChumskyLayout>> {
    match kirin_derive_core::ir::Input::<ChumskyLayout>::from_derive_input(ast) {
        Ok(input) => return Ok(input),
        Err(err) if !is_missing_type_error(&err) => return Err(err),
        Err(_) => {}
    }

    // Value-only definitions can omit #[kirin(type = ...)].
    // Inject a placeholder type so shared IR input parsing can proceed.
    let mut patched = ast.clone();
    patched.attrs.push(syn::parse_quote!(#[kirin(type = bool)]));
    let input = kirin_derive_core::ir::Input::<ChumskyLayout>::from_derive_input(&patched)?;

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

fn input_requires_ir_type(input: &kirin_derive_core::ir::Input<ChumskyLayout>) -> bool {
    match &input.data {
        kirin_derive_core::ir::Data::Struct(data) => statement_requires_ir_type(&data.0),
        kirin_derive_core::ir::Data::Enum(data) => {
            data.variants.iter().any(statement_requires_ir_type)
        }
    }
}

fn statement_requires_ir_type(stmt: &kirin_derive_core::ir::Statement<ChumskyLayout>) -> bool {
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
