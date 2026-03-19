//! Validation for format strings and field usage.

use std::collections::HashSet;

use kirin_derive_toolkit::ir::Statement;
use kirin_derive_toolkit::ir::fields::{FieldCategory, FieldInfo};
use kirin_lexer::Token;

use crate::ChumskyLayout;
use crate::field_kind::FieldCategoryExt;
use crate::format::{Format, FormatOption};
use crate::visitor::FormatVisitor;

/// Validates a format string against collected fields.
pub fn validate_format<'ir>(
    stmt: &'ir Statement<ChumskyLayout>,
    format: &Format<'_>,
    collected: &'ir [FieldInfo<ChumskyLayout>],
) -> syn::Result<ValidationResult<'ir>> {
    ValidationVisitor::new().validate(stmt, format, collected)
}

/// Whether the format string uses new-format (generic result names) or legacy mode.
///
/// Detection: if NO `ResultValue` field has a `:name` occurrence in the format string,
/// the format is new-format. If any `ResultValue` field has a `:name` or default
/// occurrence, it is legacy mode (result names are parsed by the dialect).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    /// New format: result names are parsed generically at the statement level.
    /// ResultValue fields may have `{field:type}` or no occurrence at all.
    New,
    /// Legacy format: result names are parsed by the dialect format string.
    /// At least one ResultValue field has `{field}` or `{field:name}`.
    Legacy,
}

/// Result of validation containing field occurrences.
#[derive(Debug)]
pub struct ValidationResult<'a> {
    /// Field occurrences in format string order
    pub occurrences: Vec<FieldOccurrence<'a>>,
    /// Detected format mode (new vs legacy)
    pub format_mode: FormatMode,
}

/// Represents an occurrence of a field in the format string.
#[derive(Debug, Clone)]
pub struct FieldOccurrence<'a> {
    /// The collected field info.
    pub field: &'a FieldInfo<ChumskyLayout>,
    /// The format option for this occurrence.
    pub option: FormatOption,
    /// The unique variable name for this occurrence.
    pub var_name: syn::Ident,
}

/// Visitor that validates format string usage.
pub struct ValidationVisitor<'ir> {
    /// The statement being validated (set in enter_statement)
    stmt_span: proc_macro2::Span,
    /// Field occurrences found so far
    occurrences: Vec<FieldOccurrence<'ir>>,
    /// Set of field indices that have default occurrences
    default_occurrences: HashSet<usize>,
    /// Set of field indices that have been referenced
    referenced_fields: HashSet<usize>,
    /// Fields that have name occurrence (default or :name)
    name_occurrences: HashSet<usize>,
    /// ResultValue fields that have name occurrence (default or :name)
    result_name_occurrences: HashSet<usize>,
    /// Accumulated errors
    errors: Vec<syn::Error>,
}

impl<'ir> ValidationVisitor<'ir> {
    /// Creates a new validation visitor.
    pub fn new() -> Self {
        Self {
            stmt_span: proc_macro2::Span::call_site(),
            occurrences: Vec::new(),
            default_occurrences: HashSet::new(),
            referenced_fields: HashSet::new(),
            name_occurrences: HashSet::new(),
            result_name_occurrences: HashSet::new(),
            errors: Vec::new(),
        }
    }

    /// Validates and returns the result, or an error.
    pub fn validate(
        mut self,
        stmt: &'ir Statement<ChumskyLayout>,
        format: &Format<'_>,
        collected: &'ir [FieldInfo<ChumskyLayout>],
    ) -> syn::Result<ValidationResult<'ir>> {
        crate::visitor::visit_format(&mut self, stmt, format, collected)?;

        // Detect format mode: if no ResultValue field has a name occurrence, use new-format
        let format_mode = self.detect_format_mode(collected);

        // Post-validation: check all required fields are present
        for field in collected {
            let is_result = field.category() == FieldCategory::Result;
            let is_new_format_result = is_result && format_mode == FormatMode::New;

            if !self.referenced_fields.contains(&field.index) && !field.has_default() {
                // In new-format mode, ResultValue fields are allowed to have no occurrence
                // (names are parsed generically, types use auto-placeholder)
                if !is_new_format_result {
                    self.add_error(format!(
                        "field '{}' is not mentioned in the format string. \
                         All fields must appear in the format string unless they have a default value. \
                         Use {{{}}} or {{{}:name}}/{{{}:type}} to include this field, \
                         or add #[kirin(default)] or #[kirin(default = expr)] to provide a default value.",
                        field, field, field, field
                    ));
                }
            }

            // Validate SSA/Result fields have name occurrence
            // In new-format mode, ResultValue fields don't need name occurrences
            // (names are parsed generically at the statement level)
            if field.category().is_ssa_like()
                && self.referenced_fields.contains(&field.index)
                && !self.name_occurrences.contains(&field.index)
                && !is_new_format_result
            {
                self.add_error(format!(
                    "SSA/Result field '{}' must have {{{}}} or {{{}:name}} in the format string. \
                     Using only {{{}:type}} is not sufficient because the name cannot be inferred.",
                    field, field, field, field
                ));
            }
        }

        if self.errors.is_empty() {
            Ok(ValidationResult {
                occurrences: self.occurrences,
                format_mode,
            })
        } else {
            // Combine all errors
            let mut iter = self.errors.into_iter();
            let mut combined = iter.next().unwrap();
            for err in iter {
                combined.combine(err);
            }
            Err(combined)
        }
    }

    /// Generates a unique variable name for a field occurrence.
    fn generate_var_name(
        &self,
        field: &FieldInfo<ChumskyLayout>,
        option: &FormatOption,
    ) -> syn::Ident {
        match option {
            FormatOption::Name => {
                syn::Ident::new(&format!("{}_name", field), proc_macro2::Span::call_site())
            }
            FormatOption::Type => {
                syn::Ident::new(&format!("{}_type", field), proc_macro2::Span::call_site())
            }
            FormatOption::Default => field.ident.clone().unwrap_or_else(|| {
                syn::Ident::new(&format!("{}", field), proc_macro2::Span::call_site())
            }),
        }
    }

    /// Detects format mode based on whether any ResultValue field has a name occurrence.
    fn detect_format_mode(&self, collected: &[FieldInfo<ChumskyLayout>]) -> FormatMode {
        // Check if there are any ResultValue fields at all
        let has_result_fields = collected
            .iter()
            .any(|f| f.category() == FieldCategory::Result);

        if !has_result_fields {
            // No ResultValue fields — mode doesn't matter, but default to New
            return FormatMode::New;
        }

        // If any ResultValue field has a name occurrence (default or :name), it's legacy
        if self.result_name_occurrences.is_empty() {
            FormatMode::New
        } else {
            FormatMode::Legacy
        }
    }

    fn add_error(&mut self, msg: impl std::fmt::Display) {
        self.errors
            .push(syn::Error::new(self.stmt_span, msg.to_string()));
    }
}

impl Default for ValidationVisitor<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ir> FormatVisitor<'ir> for ValidationVisitor<'ir> {
    fn enter_statement(
        &mut self,
        stmt: &'ir Statement<ChumskyLayout>,
        _format: &Format<'_>,
    ) -> syn::Result<()> {
        self.stmt_span = stmt.name.span();
        Ok(())
    }

    fn visit_field_occurrence(
        &mut self,
        field: &'ir FieldInfo<ChumskyLayout>,
        option: &FormatOption,
    ) -> syn::Result<()> {
        // Track that this field was referenced
        self.referenced_fields.insert(field.index);

        // Validate that :name and :type options are only used on SSA/Result fields
        if matches!(option, FormatOption::Name | FormatOption::Type)
            && !field.category().is_ssa_like()
        {
            let option_name = match option {
                FormatOption::Name => ":name",
                FormatOption::Type => ":type",
                FormatOption::Default => unreachable!(),
            };
            self.add_error(format!(
                "format option '{}' cannot be used on {} field '{}'. \
                 The :name and :type options are only valid for SSAValue and ResultValue fields.",
                option_name,
                field.category().ast_kind_name(),
                field
            ));
            return Ok(());
        }

        // Check for duplicate default occurrences
        if matches!(option, FormatOption::Default) {
            if self.default_occurrences.contains(&field.index) {
                self.add_error(format!(
                    "field '{}' appears multiple times with default format option. \
                     Each field can only have one default occurrence. \
                     Use {{{}:name}} or {{{}:type}} for additional occurrences.",
                    field, field, field
                ));
                return Ok(());
            }
            self.default_occurrences.insert(field.index);
        }

        // Track name occurrences for SSA/Result field validation
        if matches!(option, FormatOption::Default | FormatOption::Name) {
            self.name_occurrences.insert(field.index);
            // Also track specifically for ResultValue fields (for format mode detection)
            if field.category() == FieldCategory::Result {
                self.result_name_occurrences.insert(field.index);
            }
        }

        // Generate variable name and add occurrence
        let var_name = self.generate_var_name(field, option);
        self.occurrences.push(FieldOccurrence {
            field,
            option: option.clone(),
            var_name,
        });

        Ok(())
    }

    fn visit_tokens(&mut self, _tokens: &[Token<'_>]) -> syn::Result<()> {
        // No validation needed for tokens
        Ok(())
    }

    fn visit_default_field(&mut self, _field: &'ir FieldInfo<ChumskyLayout>) -> syn::Result<()> {
        // Fields with defaults that aren't in format are fine
        Ok(())
    }

    fn exit_statement(&mut self, _stmt: &'ir Statement<ChumskyLayout>) -> syn::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{ChumskyFieldAttrs, ChumskyStatementAttrs};
    use kirin_derive_toolkit::ir::StatementOptions;
    use kirin_derive_toolkit::ir::fields::{Collection, FieldData};

    /// Helper to build a minimal Statement<ChumskyLayout> for tests.
    fn make_stmt(fields: Vec<FieldInfo<ChumskyLayout>>) -> Statement<ChumskyLayout> {
        Statement {
            name: syn::Ident::new("TestOp", proc_macro2::Span::call_site()),
            attrs: StatementOptions {
                format: None,
                builder: None,
                constant: false,
                pure: false,
                speculatable: false,
                terminator: false,
                edge: false,
            },
            fields,
            wraps: None,
            extra: (),
            extra_attrs: ChumskyStatementAttrs { format: None },
            raw_attrs: vec![],
        }
    }

    fn make_argument(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Argument {
                ssa_type: syn::parse_quote!(Placeholder::placeholder()),
            },
        }
    }

    fn make_result(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Result {
                ssa_type: syn::parse_quote!(MyType),
                is_auto_placeholder: false,
            },
        }
    }

    fn make_value(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Value {
                ty: syn::parse_quote!(i64),
                default: None,
                into: false,
                extra: ChumskyFieldAttrs {},
            },
        }
    }

    #[test]
    fn new_format_detected_when_result_has_only_type() {
        // Format: "$h {qubit} -> {result:type}"
        // ResultValue field "result" only has :type occurrence -> new-format
        let fields = vec![make_argument(0, "qubit"), make_result(1, "result")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$h {qubit} -> {result:type}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::New);
    }

    #[test]
    fn legacy_format_detected_when_result_has_name() {
        // Format: "{result:name} = {.add} {lhs}, {rhs} -> {result:type}"
        // ResultValue field "result" has :name occurrence -> legacy
        let fields = vec![
            make_argument(0, "lhs"),
            make_argument(1, "rhs"),
            make_result(2, "result"),
        ];
        let stmt = make_stmt(fields.clone());
        let format =
            Format::parse("{result:name} = {.add} {lhs}, {rhs} -> {result:type}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::Legacy);
    }

    #[test]
    fn legacy_format_detected_when_result_has_default() {
        // Format: "{result} = {.add} {lhs}, {rhs}"
        // ResultValue field "result" has default occurrence (which implies name) -> legacy
        let fields = vec![
            make_argument(0, "lhs"),
            make_argument(1, "rhs"),
            make_result(2, "result"),
        ];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{result} = {.add} {lhs}, {rhs}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::Legacy);
    }

    #[test]
    fn new_format_no_result_fields() {
        // Format: "$ret {value}"
        // No ResultValue fields -> defaults to New
        let fields = vec![make_argument(0, "value")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$ret {value}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::New);
    }

    #[test]
    fn new_format_result_not_in_format_is_valid() {
        // Format: "$h {qubit}"
        // ResultValue field "result" has no occurrence at all -> new-format, valid
        // (auto-placeholder for type, names from generic parser)
        let fields = vec![make_argument(0, "qubit"), make_result(1, "result")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$h {qubit}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::New);
    }

    #[test]
    fn new_format_multi_result_type_only() {
        // Format: "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}"
        // Two ResultValue fields with only :type -> new-format
        let fields = vec![
            make_argument(0, "ctrl"),
            make_argument(1, "tgt"),
            make_result(2, "ctrl_out"),
            make_result(3, "tgt_out"),
        ];
        let stmt = make_stmt(fields.clone());
        let format =
            Format::parse("$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.format_mode, FormatMode::New);
    }
}
