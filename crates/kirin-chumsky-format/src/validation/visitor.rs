//! ValidationVisitor implementation.

use std::collections::HashSet;

use kirin_derive_core::ir::Statement;
use kirin_lexer::Token;

use crate::ChumskyLayout;
use kirin_derive_core::ir::fields::FieldInfo;

use crate::field_kind::FieldKind;
use crate::format::{Format, FormatOption};
use crate::visitor::FormatVisitor;

use super::result::{FieldOccurrence, ValidationResult};

/// Visitor that validates format string usage.
///
/// This performs all validation checks during format traversal,
/// collecting field occurrences along the way.
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

        // Post-validation: check all required fields are present
        for field in collected {
            if !self.referenced_fields.contains(&field.index) && !field.has_default() {
                self.add_error(format!(
                    "field '{}' is not mentioned in the format string. \
                     All fields must appear in the format string unless they have a default value. \
                     Use {{{}}} or {{{}:name}}/{{{}:type}} to include this field, \
                     or add #[kirin(default)] or #[kirin(default = expr)] to provide a default value.",
                    field, field, field, field
                ));
            }

            // Validate SSA/Result fields have name occurrence
            let kind = FieldKind::from_field_info(field);
            if kind.supports_name_type_options()
                && self.referenced_fields.contains(&field.index)
                && !self.name_occurrences.contains(&field.index)
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
        let kind = FieldKind::from_field_info(field);
        if matches!(option, FormatOption::Name | FormatOption::Type)
            && !kind.supports_name_type_options()
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
                kind.name(),
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
        // Post-validation is done in validate() after visit_format completes
        // because we need access to `collected` which isn't available here
        Ok(())
    }
}
