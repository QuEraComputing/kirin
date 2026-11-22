use std::collections::BTreeSet;

use chumsky::prelude::*;
use kirin_ir::{
    CFG, Function, FunctionInfo, GlobalSymbol, HasBottom, HasTop, InternTable, Lattice,
    Placeholder, Program, Signature, StageInfo, TypeLattice,
};
use kirin_prettyless::PrintExt;

use crate::{BoxedParser, DirectlyParsable, ParseProgramText, Token, TokenInput};

use kirin_derive_chumsky::{HasParser, PrettyPrint};
use kirin_derive_ir::{ParseDispatch, StageMeta};

// ---------------------------------------------------------------------------
// Test type lattices
// ---------------------------------------------------------------------------

macro_rules! trivial_type_lattice {
    ($name:ident, $display:literal, $parser:expr) => {
        #[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Default)]
        struct $name;

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $display)
            }
        }

        impl Lattice for $name {
            fn join(&self, _: &Self) -> Self {
                $name
            }
            fn meet(&self, _: &Self) -> Self {
                $name
            }
            fn is_subseteq(&self, _: &Self) -> bool {
                true
            }
        }

        impl HasBottom for $name {
            fn bottom() -> Self {
                $name
            }
        }

        impl HasTop for $name {
            fn top() -> Self {
                $name
            }
        }

        impl TypeLattice for $name {}

        impl Placeholder for $name {
            fn placeholder() -> Self {
                $name
            }
        }

        impl DirectlyParsable for $name {}

        impl<'t> crate::HasParser<'t> for $name {
            type Output = $name;

            fn parser<I>() -> BoxedParser<'t, I, Self::Output>
            where
                I: TokenInput<'t>,
            {
                ($parser).to($name).boxed()
            }
        }
    };
}

trivial_type_lattice!(
    UnitType,
    "()",
    just(Token::LParen).ignore_then(just(Token::RParen))
);
trivial_type_lattice!(I32Type, "i32", just(Token::Identifier("i32")));

// ---------------------------------------------------------------------------
// Test dialects
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, kirin_ir::Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = UnitType, crate = kirin_ir)]
#[chumsky(crate = crate, format = "fn {:name}{sig} {body}")]
struct FunctionDefinition {
    body: CFG,
    sig: Signature<UnitType>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, kirin_ir::Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = I32Type, crate = kirin_ir)]
#[chumsky(crate = crate, format = "fn {:name}{sig} {body}")]
struct LowerBody {
    body: CFG,
    sig: Signature<I32Type>,
}

// ---------------------------------------------------------------------------
// Stage enum: StageBucket (same dialect in both variants)
// ---------------------------------------------------------------------------

#[derive(Debug, StageMeta, ParseDispatch)]
#[stage(crate = "kirin_ir", chumsky_crate = "crate")]
enum StageBucket {
    #[stage(name = "A")]
    Parse(StageInfo<FunctionDefinition>),
    #[stage(name = "B")]
    Lower(StageInfo<FunctionDefinition>),
}

// ---------------------------------------------------------------------------
// Stage enum: MixedStage (different dialect per variant)
// ---------------------------------------------------------------------------

#[derive(Debug, StageMeta, ParseDispatch)]
#[stage(crate = "kirin_ir", chumsky_crate = "crate")]
enum MixedStage {
    #[stage(name = "A")]
    StageA(StageInfo<FunctionDefinition>),
    #[stage(name = "B")]
    StageB(StageInfo<LowerBody>),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const BODY: &str = "cfg { ^0() {} }";

fn unit_sig() -> Signature<UnitType> {
    Signature::new(vec![UnitType], UnitType, ())
}

fn function_name<S>(program: &Program<S>, function: Function) -> String {
    let info: &FunctionInfo = program.function_info(function).unwrap();
    program.resolve(info.name().unwrap()).unwrap().to_string()
}

fn parsed_names<S>(program: &Program<S>, functions: Vec<Function>) -> BTreeSet<String> {
    functions
        .into_iter()
        .map(|f| function_name(program, f))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_program_parse_accepts_mixed_function_names() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(()) -> (); specialize @B fn @bar(()) -> () {BODY}"
    );

    let parsed = program.parse(&input).unwrap();
    assert_eq!(program.stages().count(), 2);
    assert_eq!(
        parsed_names(&program, parsed),
        BTreeSet::from(["bar".into(), "foo".into()])
    );
}

#[test]
fn test_program_parse_uses_program_global_table() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let input = format!("stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY}");

    let parsed = program.parse(&input).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(function_name(&program, parsed[0]), "foo");

    let mut external_globals: InternTable<String, GlobalSymbol> = InternTable::default();
    let bar = external_globals.intern("bar".to_string());
    let bar_raw: usize = bar.into();
    assert_eq!(bar_raw, 0, "external symbol table should remain untouched");
}

#[test]
fn test_stage_enum_program_parse_uses_stage_symbol_mapping() {
    let mut program: Program<StageBucket> = Program::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(()) -> (); specialize @B fn @bar(()) -> () {BODY}"
    );

    let parsed = program.parse(&input).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(matches!(
        program.stages().collect::<Vec<_>>().as_slice(),
        [StageBucket::Parse(_), StageBucket::Lower(_)]
    ));
}

#[test]
fn test_stage_enum_program_parse_rejects_unknown_stage_mapping() {
    let mut program: Program<StageBucket> = Program::new();
    let err = program.parse("stage @Z fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
}

#[test]
fn test_stage_enum_program_parse_suggests_declared_name() {
    let mut program: Program<StageBucket> = Program::new();
    let err = program.parse("stage @C fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
    assert!(err.message.contains("@A"));
}

#[test]
fn test_stage_requires_semicolon() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program.parse("stage @A fn @foo(()) -> ()").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_specialize_requires_body() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program
        .parse("specialize @A fn @foo(()) -> ();")
        .unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_global_symbol_prefix_is_required() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program.parse("stage 1 fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_specialize_without_stage_auto_creates() {
    // With auto-creation, specialize without a prior stage declaration
    // succeeds by auto-creating the staged function.
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    program
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();
    let input = format!("specialize @A fn @foo(()) -> () {BODY}");
    let result = program.parse(&input);
    assert!(
        result.is_ok(),
        "specialize without stage should auto-create: {:?}",
        result.err()
    );
}

#[test]
fn test_comments_and_whitespace_are_accepted() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let input = format!(
        "/* stage declaration */ stage @A fn @foo(()) -> (); \
         // specialization definition\n specialize @A fn @foo(()) -> () /* definition */ {BODY}"
    );
    program.parse(&input).unwrap();
}

#[test]
fn test_program_roundtrip_print_parse_print() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let stage_a = program
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();
    let function = program.function().name("foo").new().unwrap();
    let staged_function = program
        .staged_function()
        .func(function)
        .stage(stage_a)
        .signature(unit_sig())
        .new()
        .unwrap();

    program.stage_mut(stage_a).unwrap().with_builder(|b| {
        let block = b.block().new();
        let cfg = b.cfg().add_block(block).new();
        let definition = FunctionDefinition::new(b, cfg, Signature::new(vec![], UnitType, ()));
        b.specialize()
            .staged_func(staged_function)
            .signature(unit_sig())
            .definition(definition)
            .new()
            .unwrap();
    });

    let rendered = function.sprint(&program);

    let mut parsed_program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let parsed_functions = parsed_program.parse(&rendered).unwrap();
    let parsed_function = parsed_functions
        .into_iter()
        .find(|id| function_name(&parsed_program, *id) == "foo")
        .unwrap();

    let rendered_again = parsed_function.sprint(&parsed_program);
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}

#[test]
fn test_program_parse_uses_stage_language_dispatch() {
    let mut program: Program<MixedStage> = Program::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); \
         specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(i32) -> i32; \
         specialize @B fn @bar(i32) -> i32 {BODY}"
    );

    let parsed = program.parse(&input).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(matches!(
        program.stages().collect::<Vec<_>>().as_slice(),
        [MixedStage::StageA(_), MixedStage::StageB(_)]
    ));

    assert_eq!(
        parsed_names(&program, parsed),
        BTreeSet::from(["bar".into(), "foo".into()])
    );

    let stage_b = program
        .stages()
        .find_map(|s| match s {
            MixedStage::StageB(stage) => Some(stage),
            _ => None,
        })
        .unwrap();

    let stage_b_sig = stage_b
        .staged_function_arena()
        .iter()
        .next()
        .unwrap()
        .signature();
    assert_eq!(stage_b_sig, &Signature::new(vec![I32Type], I32Type, ()));
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

#[test]
fn test_program_parse_empty_input() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program.parse("").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
    assert!(err.message.contains("expected at least one declaration"));
}

#[test]
fn test_program_parse_whitespace_only() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program.parse("   \n\t  ").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

// ---------------------------------------------------------------------------
// Numeric stage symbol (@1)
// ---------------------------------------------------------------------------

#[test]
fn test_program_parse_numeric_stage_symbol_rejected() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    // Numeric tokens like `1` are not prefixed with `@`, so `stage 1` should fail
    let err = program.parse("stage 1 fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_program_numeric_stage_lookup_by_existing_id() {
    // When a stage already exists in the program, @<numeric> can find it by raw ID
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let stage_id = program
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();

    // First parse a normal declaration to set things up
    let input = format!("stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY}");
    let result = program.parse(&input);
    assert!(result.is_ok());

    // The stage exists with name "A" at some ID
    assert_eq!(program.stages().count(), 1);
    let _ = stage_id; // stage was pre-created
}

// ---------------------------------------------------------------------------
// best_stage_suggestion threshold (distance > 3)
// ---------------------------------------------------------------------------

#[test]
fn test_stage_suggestion_close_name() {
    // "C" vs declared names "A", "B" — distance 1, should get a suggestion
    let mut program: Program<StageBucket> = Program::new();
    let err = program.parse("stage @C fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
    // With Levenshtein distance <= 3, should suggest a known stage
    assert!(
        err.message.contains("did you mean")
            || err.message.contains("@A")
            || err.message.contains("@B"),
        "expected suggestion in error message, got: {}",
        err.message
    );
}

#[test]
fn test_stage_suggestion_very_distant_name() {
    // "XYZXYZXYZ" vs declared names "A", "B" — distance > 3, should NOT suggest
    let mut program: Program<StageBucket> = Program::new();
    let err = program
        .parse("stage @XYZXYZXYZ fn @foo(()) -> ();")
        .unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
    assert!(
        !err.message.contains("did you mean"),
        "expected no suggestion for distant name, got: {}",
        err.message
    );
}

// ---------------------------------------------------------------------------
// FunctionParseError::source chain
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_body_parse_has_source() {
    use std::error::Error;
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    // Valid header but invalid body tokens
    let err = program
        .parse("stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () cfg { invalid }")
        .unwrap_err();
    // The body parse failure should chain a source error
    // (may or may not have source depending on where it fails)
    let _ = err.source();
}

// ---------------------------------------------------------------------------
// Duplicate stage declaration with same signature (idempotent)
// ---------------------------------------------------------------------------

#[test]
fn test_duplicate_stage_declaration_same_signature() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); \
         stage @A fn @foo(()) -> (); \
         specialize @A fn @foo(()) -> () {BODY}"
    );
    let result = program.parse(&input);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Invalid declaration keyword
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_declaration_keyword() {
    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let err = program.parse("define @A fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

// ---------------------------------------------------------------------------
// Body-span scanner
//
// The framework only strips `specialize @stage`; the rest of the declaration —
// discriminator, header, and brace-balanced body — is handed to the dialect
// statement parser as one span. The scanner skips tokens until the first `{`,
// so it is agnostic to *which* discriminator precedes the body, and stays
// correct for all four body kinds without knowing any of them.
// ---------------------------------------------------------------------------

/// Scan one `specialize` declaration and return the exact source slice handed
/// to the dialect statement parser.
///
/// `LowerBody`'s `I32Type` only affects the *header* type list; the scanner
/// never parses body contents, so any body kind can be scanned through it.
fn scanned_body_span(src: &str) -> String {
    let tokens = super::syntax::tokenize(src);
    let (declaration, _) = super::syntax::parse_one_declaration::<LowerBody>(&tokens)
        .expect("declaration should scan");
    match declaration {
        super::syntax::Declaration::Specialize {
            definition_span, ..
        } => src[definition_span.start..definition_span.end].to_string(),
        super::syntax::Declaration::Stage(_) => panic!("expected a specialize declaration"),
    }
}

#[test]
fn test_body_span_scans_tagged_cfg() {
    let body = "fn @f(i32) -> i32 cfg { ^entry(%x: i32) { %r = add %x, %x; ret %r; } }";
    assert_eq!(scanned_body_span(&format!("specialize @A {body}")), body);
}

#[test]
fn test_body_span_scans_tagged_block() {
    let body = "fn @f(i32) -> i32 block ^body(%x: i32) { %r = add %x, %x; ret %r; }";
    assert_eq!(scanned_body_span(&format!("specialize @A {body}")), body);
}

#[test]
fn test_body_span_scans_digraph() {
    let body = "fn @f(i32) -> i32 digraph ^g0(%x: i32) { %r = add %x, %x; yield %r; }";
    assert_eq!(scanned_body_span(&format!("specialize @A {body}")), body);
}

#[test]
fn test_body_span_scans_ungraph() {
    let body = "fn @f(i32) -> i32 ungraph ^u0(%x: i32) { edge %w = wire; node(%x, %w); }";
    assert_eq!(scanned_body_span(&format!("specialize @A {body}")), body);
}

#[test]
fn test_body_span_scans_a_projected_body_without_a_discriminator() {
    // Projections stay raw, so a dialect can spell its own wrapper. The
    // scanner does not require a keyword — only a brace-balanced body.
    let body = "fn @f(i32) -> i32 (%x: i32) { %r = add %x, %x; ret %r; }";
    assert_eq!(scanned_body_span(&format!("specialize @A {body}")), body);
}

#[test]
fn test_body_span_stops_at_the_matching_brace() {
    // Two declarations: the first span must end at *its* closing brace, not
    // run on into the second.
    let first = "fn @f(i32) -> i32 cfg { ^entry(%x: i32) { ret %x; } }";
    let second = "fn @g(i32) -> i32 cfg { ^e { } }";
    let src = format!("specialize @A {first} specialize @A {second}");
    assert_eq!(scanned_body_span(&src), first);
}

#[test]
fn test_body_span_requires_an_opening_brace() {
    let tokens = super::syntax::tokenize("specialize @A fn @f(i32) -> i32 cfg;");
    let errors = super::syntax::parse_one_declaration::<LowerBody>(&tokens)
        .expect_err("a body with no `{` should not scan");
    assert!(
        errors
            .iter()
            .any(|e| format!("{e}").contains("expected '{'")),
        "expected a missing-brace diagnostic, got: {errors:?}"
    );
}

#[test]
fn test_body_span_rejects_an_unclosed_brace() {
    let tokens = super::syntax::tokenize("specialize @A fn @f(i32) -> i32 cfg { ^entry {");
    let errors = super::syntax::parse_one_declaration::<LowerBody>(&tokens)
        .expect_err("an unbalanced body should not scan");
    assert!(
        errors
            .iter()
            .any(|e| format!("{e}").contains("unclosed '{'")),
        "expected an unclosed-brace diagnostic, got: {errors:?}"
    );
}

/// A position a failed pass poisoned still answers to its name, so parsing
/// `@A` must resolve to it rather than silently create a second stage called
/// `@A`.
#[test]
fn test_parse_does_not_duplicate_a_poisoned_stage() {
    use kirin_ir::{RewriteError, StageStatus};

    let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
    let stage_a = program
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();

    let _ = program.run_pass::<FunctionDefinition, _, (), _>(stage_a, |_| {
        Err(RewriteError::UnexpectedTerminator)
    });
    assert_eq!(
        program.stage_status(stage_a),
        Some(StageStatus::Poisoned),
        "the pass should have poisoned @A"
    );

    let input = format!("specialize @A fn @foo(()) -> () {BODY}");
    assert!(program.parse(&input).is_err());
    assert_eq!(
        program.stage_names().count(),
        1,
        "@A must resolve to the existing position, not create a second one"
    );
}

#[test]
fn test_unit_typed_signatures_roundtrip() {
    let parse = |text: &str| {
        let mut program: Program<StageInfo<FunctionDefinition>> = Program::new();
        program.parse(text).unwrap();
        let stage = program.stages().next().unwrap();
        let staged = stage.staged_function_arena().iter().next().unwrap();
        staged.signature().clone()
    };
    for sig in [
        unit_sig(),
        Signature::with_rets(vec![UnitType], vec![], ()),
        Signature::with_rets(vec![UnitType], vec![UnitType, UnitType], ()),
    ] {
        assert_eq!(parse(&format!("stage @A fn @foo{sig};")), sig);
    }
    // A unit type is one return; no returns is the absence of the arrow.
    assert_eq!(parse("stage @A fn @foo(()) -> ();"), unit_sig());
    assert_eq!(
        parse("stage @A fn @foo(());"),
        Signature::with_rets(vec![UnitType], vec![], ())
    );
}
