//! Tests for deriving parser/pretty-print directly on compile-time value enums.

use kirin::ir::{Dialect, StageInfo};
use kirin_chumsky::prelude::Document;
use kirin_chumsky::{HasParser, PrettyPrint, parse_ast};
use kirin_prettyless::Config;
use kirin_test_languages::SimpleType;

#[derive(Debug, Clone, PartialEq, HasParser, PrettyPrint)]
#[chumsky(crate = kirin_chumsky)]
enum CompileTimeTestValue {
    #[chumsky(format = "foo({0}, {1})")]
    Foo(i32, u32),
    #[chumsky(format = "goo({a}, {b})")]
    Goo { a: i32, b: String },
}

// A small dialect used only to provide document context for pretty printing tests.
#[derive(Debug, Clone, PartialEq, Dialect, PrettyPrint)]
#[kirin(type = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
#[allow(dead_code)]
enum RenderDialect {
    #[chumsky(format = "nop {0}")]
    Nop(i32),
}

fn render(value: &CompileTimeTestValue) -> String {
    let stage: StageInfo<RenderDialect> = StageInfo::default();
    let doc = Document::new(Config::default(), &stage);
    let arena_doc = value.pretty_print(&doc);

    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render should succeed");
    output
}

#[test]
fn test_parse_tuple_variant_without_kirin_type_attr() {
    let ast = parse_ast::<CompileTimeTestValue>("foo(1, 2)").expect("parse failed");
    match ast.0 {
        CompileTimeTestValueAST::Foo(a, b) => {
            assert_eq!(a, 1);
            assert_eq!(b, 2);
        }
        _ => panic!("expected Foo"),
    }
}

#[test]
fn test_parse_named_variant_without_kirin_type_attr() {
    let ast = parse_ast::<CompileTimeTestValue>("goo(-3, label)").expect("parse failed");
    match ast.0 {
        CompileTimeTestValueAST::Goo { a, b } => {
            assert_eq!(a, -3);
            assert_eq!(b, "label");
        }
        _ => panic!("expected Goo"),
    }
}

#[test]
fn test_pretty_print_tuple_variant() {
    let value = CompileTimeTestValue::Foo(1, 2);
    assert_eq!(render(&value), "foo (1, 2)");
}

#[test]
fn test_pretty_print_named_variant() {
    let value = CompileTimeTestValue::Goo {
        a: -3,
        b: "label".to_string(),
    };
    assert_eq!(render(&value), "goo (-3, \"label\")");
}
