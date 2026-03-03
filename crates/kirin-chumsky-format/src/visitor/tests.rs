use super::*;
use crate::ChumskyLayout;
use crate::field_kind::collect_fields;
use crate::format::{Format, FormatOption};
use kirin_derive_core::ir::{Input, Statement};
use kirin_derive_core::ir::fields::FieldInfo;
use kirin_lexer::Token;

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
        #[kirin(type = SimpleType)]
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
        #[kirin(type = SimpleType)]
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
        #[kirin(type = SimpleType)]
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
