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

/// Validates that `ir_path` is available when DiGraph/UnGraph fields use body projections.
///
/// Body projections on graph fields (`{field:ports}`, `{field:captures}`, `{field:body}`)
/// require `ir_path` (the `#[kirin(crate = ...)]` path) for pretty-print code generation.
/// Call this before pretty-print codegen to surface a proper `syn::Error` instead of a panic.
pub fn validate_ir_path_for_body_projections<L: kirin_derive_toolkit::ir::Layout>(
    format: &Format<'_>,
    collected: &[FieldInfo<L>],
    ir_path: Option<&syn::Path>,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if ir_path.is_some() {
        return Ok(());
    }

    for elem in format.elements() {
        if let crate::format::FormatElement::Field(name, crate::format::FormatOption::Body(_)) =
            elem
        {
            let field = collected.iter().find(|f| {
                f.ident.as_ref().is_some_and(|id| id == name) || f.index.to_string() == *name
            });
            if let Some(field) = field {
                if matches!(
                    field.category(),
                    FieldCategory::DiGraph | FieldCategory::UnGraph
                ) {
                    let kind = match field.category() {
                        FieldCategory::DiGraph => "DiGraph",
                        FieldCategory::UnGraph => "UnGraph",
                        _ => unreachable!(),
                    };
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "{kind} field '{field}' uses body projections which require \
                             the IR crate path for code generation. Ensure a \
                             `#[kirin(crate = ...)]` attribute is present.",
                        ),
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Result of validation containing field occurrences.
#[derive(Debug)]
pub struct ValidationResult<'a> {
    /// Field occurrences in format string order
    pub occurrences: Vec<FieldOccurrence<'a>>,
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
    /// Body projections seen per field index (for completeness checking)
    body_projections: std::collections::HashMap<usize, Vec<crate::format::BodyProjection>>,
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
            body_projections: std::collections::HashMap::new(),
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

        // Reject legacy result name usage: ResultValue fields must NOT have :name occurrences
        if !self.result_name_occurrences.is_empty() {
            self.add_error(
                "Result names are parsed generically. Remove `{result:name} =` from your \
                 format string and use `$keyword` syntax. ResultValue fields should only \
                 have `{field:type}` or no occurrence at all.",
            );
        }

        // Post-validation: check all required fields are present
        for field in collected {
            let is_result = field.category() == FieldCategory::Result;

            if !self.referenced_fields.contains(&field.index) && !field.has_default() {
                // ResultValue fields are allowed to have no occurrence
                // (names are parsed generically, types use auto-placeholder)
                if !is_result {
                    self.add_error(format!(
                        "field '{}' is not mentioned in the format string. \
                         All fields must appear in the format string unless they have a default value. \
                         Use {{{}}} or {{{}:type}} to include this field, \
                         or add #[kirin(default)] or #[kirin(default = expr)] to provide a default value.",
                        field, field, field
                    ));
                }
            }

            // Validate SSA fields have name occurrence (Result fields are handled generically)
            if field.category().is_ssa_like()
                && !is_result
                && self.referenced_fields.contains(&field.index)
                && !self.name_occurrences.contains(&field.index)
            {
                self.add_error(format!(
                    "SSA field '{}' must have {{{}}} or {{{}:name}} in the format string. \
                     Using only {{{}:type}} is not sufficient because the name cannot be inferred.",
                    field, field, field, field
                ));
            }
        }

        // Validate body projection completeness: when a field has ANY body projection,
        // all required projections must be present for roundtrip correctness.
        for field in collected {
            if let Some(projs) = self.body_projections.get(&field.index) {
                use crate::format::BodyProjection;
                let has = |p: BodyProjection| projs.contains(&p);
                let (required, field_kind): (&[BodyProjection], &str) = match field.category() {
                    FieldCategory::DiGraph => (
                        &[
                            BodyProjection::Ports,
                            BodyProjection::Captures,
                            BodyProjection::Body,
                        ],
                        "DiGraph",
                    ),
                    FieldCategory::UnGraph => (
                        &[
                            BodyProjection::Ports,
                            BodyProjection::Captures,
                            BodyProjection::Body,
                        ],
                        "UnGraph",
                    ),
                    FieldCategory::Block => {
                        (&[BodyProjection::Args, BodyProjection::Body], "Block")
                    }
                    FieldCategory::Region => (&[BodyProjection::Body], "Region"),
                    _ => continue,
                };
                let missing: Vec<&str> = required
                    .iter()
                    .filter(|r| !has(**r))
                    .map(|r| match r {
                        BodyProjection::Ports => ":ports",
                        BodyProjection::Captures => ":captures",
                        BodyProjection::Args => ":args",
                        BodyProjection::Body => ":body",
                    })
                    .collect();
                if !missing.is_empty() {
                    self.add_error(format!(
                        "{} field '{}' uses body projections but is missing required projection(s): {}. \
                         All structural parts must be present for roundtrip correctness.",
                        field_kind,
                        field,
                        missing.join(", "),
                    ));
                }
            }
        }

        if self.errors.is_empty() {
            Ok(ValidationResult {
                occurrences: self.occurrences,
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
            FormatOption::Body(proj) => {
                let suffix = match proj {
                    crate::format::BodyProjection::Ports => format!("{}_ports", field),
                    crate::format::BodyProjection::Captures => format!("{}_captures", field),
                    crate::format::BodyProjection::Args => format!("{}_args", field),
                    crate::format::BodyProjection::Body => format!("{}_body", field),
                };
                syn::Ident::new(&suffix, proc_macro2::Span::call_site())
            }
            FormatOption::Signature(proj) => {
                let suffix = match proj {
                    crate::format::SignatureProjection::Inputs => format!("{}_inputs", field),
                    crate::format::SignatureProjection::Return => format!("{}_return", field),
                };
                syn::Ident::new(&suffix, proc_macro2::Span::call_site())
            }
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
                FormatOption::Default | FormatOption::Body(_) | FormatOption::Signature(_) => {
                    unreachable!()
                }
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

        // Validate body projections against field category
        if let FormatOption::Body(proj) = option {
            use crate::format::BodyProjection;
            let category = field.category();
            let valid = match proj {
                BodyProjection::Ports | BodyProjection::Captures => {
                    matches!(category, FieldCategory::DiGraph | FieldCategory::UnGraph)
                }
                BodyProjection::Args => {
                    matches!(category, FieldCategory::Block)
                }
                BodyProjection::Body => {
                    matches!(
                        category,
                        FieldCategory::DiGraph
                            | FieldCategory::UnGraph
                            | FieldCategory::Region
                            | FieldCategory::Block
                    )
                }
            };

            if !valid {
                let proj_name = match proj {
                    BodyProjection::Ports => ":ports",
                    BodyProjection::Captures => ":captures",
                    BodyProjection::Args => ":args",
                    BodyProjection::Body => ":body",
                };
                let valid_on = match proj {
                    BodyProjection::Ports | BodyProjection::Captures => "DiGraph or UnGraph",
                    BodyProjection::Args => "Block",
                    BodyProjection::Body => "DiGraph, UnGraph, Region, or Block",
                };
                self.add_error(format!(
                    "'{}' projection is only valid on {} fields, but '{}' is a {} field",
                    proj_name,
                    valid_on,
                    field,
                    field.category().ast_kind_name(),
                ));
                return Ok(());
            }
        }

        // Track body projections per field for completeness checking
        if let FormatOption::Body(proj) = option {
            self.body_projections
                .entry(field.index)
                .or_default()
                .push(*proj);
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
    fn result_with_only_type_is_valid() {
        // Format: "$h {qubit} -> {result:type}"
        // ResultValue field "result" only has :type occurrence -> valid
        let fields = vec![make_argument(0, "qubit"), make_result(1, "result")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$h {qubit} -> {result:type}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.occurrences.len(), 2);
    }

    #[test]
    fn legacy_result_name_rejected() {
        // Format uses {result:name} which is now rejected
        let fields = vec![
            make_argument(0, "lhs"),
            make_argument(1, "rhs"),
            make_result(2, "result"),
        ];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$add {lhs}, {rhs} -> {result:name}", None).unwrap();

        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string()
                .contains("Result names are parsed generically"),
            "Error should reject legacy result:name: {}",
            err
        );
    }

    #[test]
    fn legacy_result_default_rejected() {
        // Format uses {result} (default occurrence for ResultValue implies name) which is now rejected
        let fields = vec![
            make_argument(0, "lhs"),
            make_argument(1, "rhs"),
            make_result(2, "result"),
        ];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$add {result}, {lhs}, {rhs}", None).unwrap();

        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string()
                .contains("Result names are parsed generically"),
            "Error should reject legacy result default: {}",
            err
        );
    }

    #[test]
    fn no_result_fields_is_valid() {
        // Format: "$ret {value}"
        // No ResultValue fields -> valid
        let fields = vec![make_argument(0, "value")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$ret {value}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.occurrences.len(), 1);
    }

    #[test]
    fn result_not_in_format_is_valid() {
        // Format: "$h {qubit}"
        // ResultValue field "result" has no occurrence at all -> valid
        // (auto-placeholder for type, names from generic parser)
        let fields = vec![make_argument(0, "qubit"), make_result(1, "result")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("$h {qubit}", None).unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.occurrences.len(), 1);
    }

    #[test]
    fn multi_result_type_only_is_valid() {
        // Format: "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}"
        // Two ResultValue fields with only :type -> valid
        let fields = vec![
            make_argument(0, "ctrl"),
            make_argument(1, "tgt"),
            make_result(2, "ctrl_out"),
            make_result(3, "tgt_out"),
        ];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse(
            "$cnot {ctrl}, {tgt} -> {ctrl_out:type}, {tgt_out:type}",
            None,
        )
        .unwrap();

        let result = validate_format(&stmt, &format, &fields).unwrap();
        assert_eq!(result.occurrences.len(), 4);
    }

    // ---- Body projection validation tests ----

    fn make_block(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Block,
        }
    }

    fn make_region(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::Region,
        }
    }

    fn make_digraph(index: usize, name: &str) -> FieldInfo<ChumskyLayout> {
        FieldInfo {
            index,
            ident: Some(syn::Ident::new(name, proc_macro2::Span::call_site())),
            collection: Collection::Single,
            data: FieldData::DiGraph,
        }
    }

    #[test]
    fn complete_digraph_projections_are_valid() {
        let fields = vec![make_digraph(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse(
            "({body:ports}) captures ({body:captures}) {{ {body:body} }}",
            None,
        )
        .unwrap();
        assert!(validate_format(&stmt, &format, &fields).is_ok());
    }

    #[test]
    fn incomplete_digraph_projections_are_invalid() {
        // Only :body without :ports and :captures
        let fields = vec![make_digraph(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:body}", None).unwrap();
        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string().contains(":ports"),
            "Error should mention missing :ports: {}",
            err
        );
    }

    #[test]
    fn body_projection_on_region_is_valid() {
        let fields = vec![make_region(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:body}", None).unwrap();
        assert!(validate_format(&stmt, &format, &fields).is_ok());
    }

    #[test]
    fn args_projection_on_block_is_valid() {
        let fields = vec![make_block(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:args} {body:body}", None).unwrap();
        assert!(validate_format(&stmt, &format, &fields).is_ok());
    }

    #[test]
    fn ports_projection_on_region_is_invalid() {
        let fields = vec![make_region(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:ports}", None).unwrap();
        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string().contains(":ports"),
            "Error should mention :ports: {}",
            err
        );
    }

    #[test]
    fn args_projection_on_region_is_invalid() {
        let fields = vec![make_region(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:args}", None).unwrap();
        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string().contains(":args"),
            "Error should mention :args: {}",
            err
        );
    }

    #[test]
    fn body_projection_on_value_is_invalid() {
        let fields = vec![make_value(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:body}", None).unwrap();
        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string().contains(":body"),
            "Error should mention :body: {}",
            err
        );
    }

    #[test]
    fn captures_projection_on_block_is_invalid() {
        let fields = vec![make_block(0, "body")];
        let stmt = make_stmt(fields.clone());
        let format = Format::parse("{body:captures}", None).unwrap();
        let err = validate_format(&stmt, &format, &fields).unwrap_err();
        assert!(
            err.to_string().contains(":captures"),
            "Error should mention :captures: {}",
            err
        );
    }

    #[test]
    fn yields_is_not_a_body_projection() {
        // :yields was removed as a body projection — it's now {:return} (context).
        // {body:yields} should fail to parse.
        let result = Format::parse("{body:yields}", None);
        assert!(
            result.is_err(),
            "yields should not be a valid body projection"
        );
    }
}
