//! FormatVisitor trait for unified format-driven traversal.
//!
//! This module provides a visitor pattern for traversing format strings
//! with field context. It's used for validation and code generation.

use std::collections::HashMap;

use kirin_derive_core::ir::Statement;
use kirin_lexer::Token;

use crate::ChumskyLayout;
use crate::format::{Format, FormatElement, FormatOption};
use kirin_derive_core::ir::fields::FieldInfo;

/// Visitor trait for format-driven traversal.
///
/// Implementors receive callbacks as the format string is traversed,
/// with full context about fields and their format options.
pub trait FormatVisitor<'ir> {
    /// Called once per statement before field iteration.
    ///
    /// Use this to initialize any state needed for processing.
    fn enter_statement(
        &mut self,
        _stmt: &'ir Statement<ChumskyLayout>,
        _format: &Format<'_>,
    ) -> syn::Result<()> {
        Ok(())
    }

    /// Called for each field occurrence in format string order.
    ///
    /// A field may appear multiple times with different options
    /// (e.g., `{x:name}` and `{x:type}`).
    fn visit_field_occurrence(
        &mut self,
        _field: &'ir FieldInfo<ChumskyLayout>,
        _option: &FormatOption,
    ) -> syn::Result<()> {
        Ok(())
    }

    /// Called for token sequences between fields.
    ///
    /// These are the literal tokens that should be matched/printed.
    fn visit_tokens(&mut self, _tokens: &[Token<'_>]) -> syn::Result<()> {
        Ok(())
    }

    /// Called for fields not in format string (have defaults).
    ///
    /// These fields won't be parsed/printed but need to be included
    /// in the AST with their default values.
    fn visit_default_field(&mut self, _field: &'ir FieldInfo<ChumskyLayout>) -> syn::Result<()> {
        Ok(())
    }

    /// Called after all elements have been processed.
    ///
    /// Use this for final validation or to collect results.
    fn exit_statement(&mut self, _stmt: &'ir Statement<ChumskyLayout>) -> syn::Result<()> {
        Ok(())
    }
}

/// Drives the visitor through a format string with field context.
///
/// This function:
/// 1. Calls `enter_statement`
/// 2. Iterates format elements, calling `visit_field_occurrence` or `visit_tokens`
/// 3. Calls `visit_default_field` for fields with defaults not in format
/// 4. Calls `exit_statement`
///
/// # Arguments
///
/// * `visitor` - The visitor implementation
/// * `stmt` - The statement being processed
/// * `format` - The parsed format string
/// * `collected` - All collected fields from the statement
///
/// # Errors
///
/// Returns the first error encountered from any visitor method.
pub fn visit_format<'ir, V: FormatVisitor<'ir>>(
    visitor: &mut V,
    stmt: &'ir Statement<ChumskyLayout>,
    format: &Format<'_>,
    collected: &'ir [FieldInfo<ChumskyLayout>],
) -> syn::Result<()> {
    // Build a map from field name/index to FieldInfo
    let field_map = build_field_map(stmt, collected);

    // Track which fields are referenced in the format
    let mut referenced_fields = std::collections::HashSet::new();

    // Enter statement
    visitor.enter_statement(stmt, format)?;

    // Visit format elements in order
    for elem in format.elements() {
        match elem {
            FormatElement::Token(tokens) => {
                visitor.visit_tokens(tokens)?;
            }
            FormatElement::Field(name, option) => {
                if let Some(field) = field_map.get(*name) {
                    referenced_fields.insert(field.index);
                    visitor.visit_field_occurrence(field, option)?;
                }
                // Note: Unknown fields are not an error here - validation handles that
            }
        }
    }

    // Visit fields with defaults that weren't in the format
    for field in collected {
        if !referenced_fields.contains(&field.index) && field.has_default() {
            visitor.visit_default_field(field)?;
        }
    }

    // Exit statement
    visitor.exit_statement(stmt)?;

    Ok(())
}

/// Builds a map from field name (string or index) to FieldInfo.
fn build_field_map<'a>(
    stmt: &Statement<ChumskyLayout>,
    collected: &'a [FieldInfo<ChumskyLayout>],
) -> HashMap<String, &'a FieldInfo<ChumskyLayout>> {
    let name_to_index = stmt.field_name_to_index();
    let mut map = HashMap::new();

    for field in collected {
        // Add by index (for positional references like {0}, {1})
        map.insert(field.index.to_string(), field);

        // Add by name if it's a named field
        if let Some(ident) = &field.ident {
            map.insert(ident.to_string(), field);
        }
    }

    // Also add names that map to indices via field_name_to_index
    for (name, idx) in name_to_index {
        if let Some(field) = collected.iter().find(|f| f.index == idx) {
            map.insert(name, field);
        }
    }

    map
}

/// Context passed to visitors that need access to the statement and format.
///
/// This is useful for visitors that need to look up additional information
/// during traversal.
pub struct VisitorContext<'ir, 'fmt> {
    /// The statement being visited
    pub stmt: &'ir Statement<ChumskyLayout>,
    /// The parsed format string
    pub format: &'fmt Format<'fmt>,
    /// Map from field name/index to field
    pub field_map: HashMap<String, &'ir FieldInfo<ChumskyLayout>>,
}

impl<'ir, 'fmt> VisitorContext<'ir, 'fmt> {
    /// Creates a new visitor context.
    pub fn new(
        stmt: &'ir Statement<ChumskyLayout>,
        format: &'fmt Format<'fmt>,
        collected: &'ir [FieldInfo<ChumskyLayout>],
    ) -> Self {
        Self {
            stmt,
            format,
            field_map: build_field_map(stmt, collected),
        }
    }

    /// Looks up a field by name or index.
    pub fn get_field(&self, name: &str) -> Option<&'ir FieldInfo<ChumskyLayout>> {
        self.field_map.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_kind::collect_fields;
    use kirin_derive_core::ir::Input;

    /// Test visitor that records all calls for verification.
    #[derive(Default)]
    struct RecordingVisitor {
        entered: bool,
        exited: bool,
        field_occurrences: Vec<(usize, FormatOption)>,
        token_sequences: Vec<usize>,
        default_fields: Vec<usize>,
    }

    impl<'ir> FormatVisitor<'ir> for RecordingVisitor {
        fn enter_statement(
            &mut self,
            _stmt: &'ir Statement<ChumskyLayout>,
            _format: &Format<'_>,
        ) -> syn::Result<()> {
            self.entered = true;
            Ok(())
        }

        fn visit_field_occurrence(
            &mut self,
            field: &'ir FieldInfo<ChumskyLayout>,
            option: &FormatOption,
        ) -> syn::Result<()> {
            self.field_occurrences.push((field.index, option.clone()));
            Ok(())
        }

        fn visit_tokens(&mut self, tokens: &[Token<'_>]) -> syn::Result<()> {
            self.token_sequences.push(tokens.len());
            Ok(())
        }

        fn visit_default_field(&mut self, field: &'ir FieldInfo<ChumskyLayout>) -> syn::Result<()> {
            self.default_fields.push(field.index);
            Ok(())
        }

        fn exit_statement(&mut self, _stmt: &'ir Statement<ChumskyLayout>) -> syn::Result<()> {
            self.exited = true;
            Ok(())
        }
    }

    #[test]
    fn test_visitor_basic_traversal() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = SimpleType)]
            #[chumsky(format = "{res} = add {lhs}, {rhs}")]
            struct Add {
                lhs: SSAValue,
                rhs: SSAValue,
                res: ResultValue,
            }
        };

        let ir_input: Input<ChumskyLayout> = Input::from_derive_input(&input).unwrap();
        let stmt = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => &s.0,
            _ => panic!("Expected struct"),
        };

        let format_str = stmt.extra_attrs.format.as_ref().unwrap();
        let format = Format::parse(format_str, None).unwrap();
        let collected = collect_fields(stmt);

        let mut visitor = RecordingVisitor::default();
        visit_format(&mut visitor, stmt, &format, &collected).unwrap();

        assert!(visitor.entered, "enter_statement should be called");
        assert!(visitor.exited, "exit_statement should be called");
        // Format: {res} = add {lhs}, {rhs}
        // Fields: res, lhs, rhs (3 field occurrences)
        assert_eq!(visitor.field_occurrences.len(), 3);
        // Tokens: "= add" and "," (2 token sequences)
        assert_eq!(visitor.token_sequences.len(), 2);
    }

    #[test]
    fn test_visitor_with_format_options() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = SimpleType)]
            #[chumsky(format = "{res:name} = neg {arg} -> {res:type}")]
            struct Neg {
                arg: SSAValue,
                res: ResultValue,
            }
        };

        let ir_input: Input<ChumskyLayout> = Input::from_derive_input(&input).unwrap();
        let stmt = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => &s.0,
            _ => panic!("Expected struct"),
        };

        let format_str = stmt.extra_attrs.format.as_ref().unwrap();
        let format = Format::parse(format_str, None).unwrap();
        let collected = collect_fields(stmt);

        let mut visitor = RecordingVisitor::default();
        visit_format(&mut visitor, stmt, &format, &collected).unwrap();

        // Format: {res:name} = neg {arg} -> {res:type}
        // Field occurrences: res:name, arg, res:type (3 total)
        assert_eq!(visitor.field_occurrences.len(), 3);

        // Check the format options
        assert!(matches!(visitor.field_occurrences[0].1, FormatOption::Name));
        assert!(matches!(
            visitor.field_occurrences[1].1,
            FormatOption::Default
        ));
        assert!(matches!(visitor.field_occurrences[2].1, FormatOption::Type));
    }

    #[test]
    fn test_visitor_context() {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[kirin(type_lattice = SimpleType)]
            #[chumsky(format = "{a} + {b}")]
            struct Add {
                a: SSAValue,
                b: SSAValue,
            }
        };

        let ir_input: Input<ChumskyLayout> = Input::from_derive_input(&input).unwrap();
        let stmt = match &ir_input.data {
            kirin_derive_core::ir::Data::Struct(s) => &s.0,
            _ => panic!("Expected struct"),
        };

        let format_str = stmt.extra_attrs.format.as_ref().unwrap();
        let format = Format::parse(format_str, None).unwrap();
        let collected = collect_fields(stmt);

        let ctx = VisitorContext::new(stmt, &format, &collected);

        // Can look up by name
        assert!(ctx.get_field("a").is_some());
        assert!(ctx.get_field("b").is_some());
        assert!(ctx.get_field("nonexistent").is_none());

        // Can look up by index
        assert!(ctx.get_field("0").is_some());
        assert!(ctx.get_field("1").is_some());
    }
}
