use std::collections::BTreeSet;

use chumsky::prelude::*;
use kirin_ir::{
    Function, FunctionInfo, GlobalSymbol, HasBottom, HasTop, InternTable, Lattice, Pipeline,
    Region, Signature, StageInfo, TypeLattice,
};
use kirin_prettyless::PrintExt;

use crate::{BoxedParser, DirectlyParsable, ParsePipelineText, Token, TokenInput};

use kirin_chumsky_derive::{HasParser, PrettyPrint};
use kirin_derive::StageMeta;

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
        impl DirectlyParsable for $name {}

        impl<'tokens, 'src: 'tokens> crate::HasParser<'tokens, 'src> for $name {
            type Output = $name;

            fn parser<I>() -> BoxedParser<'tokens, 'src, I, Self::Output>
            where
                I: TokenInput<'tokens, 'src>,
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
#[kirin(fn, type = UnitType, crate = kirin_ir)]
#[chumsky(crate = crate, format = "{body}")]
struct FunctionBody {
    body: Region,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, kirin_ir::Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = I32Type, crate = kirin_ir)]
#[chumsky(crate = crate, format = "{body}")]
struct LowerBody {
    body: Region,
}

// ---------------------------------------------------------------------------
// Stage enum: StageBucket (same dialect in both variants)
// ---------------------------------------------------------------------------

#[derive(Debug, StageMeta)]
#[stage(crate = "kirin_ir")]
enum StageBucket {
    #[stage(name = "A")]
    Parse(StageInfo<FunctionBody>),
    #[stage(name = "B")]
    Lower(StageInfo<FunctionBody>),
}

// ---------------------------------------------------------------------------
// Stage enum: MixedStage (different dialect per variant)
// ---------------------------------------------------------------------------

#[derive(Debug, StageMeta)]
#[stage(crate = "kirin_ir")]
enum MixedStage {
    #[stage(name = "A")]
    StageA(StageInfo<FunctionBody>),
    #[stage(name = "B")]
    StageB(StageInfo<LowerBody>),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const BODY: &str = "{ ^0() {} }";

fn unit_sig() -> Signature<UnitType> {
    Signature {
        params: vec![UnitType],
        ret: UnitType,
        constraints: (),
    }
}

fn function_name<S>(pipeline: &Pipeline<S>, function: Function) -> String {
    let info: &FunctionInfo = pipeline.function_info(function).unwrap();
    pipeline.resolve(info.name().unwrap()).unwrap().to_string()
}

fn parsed_names<S>(pipeline: &Pipeline<S>, functions: Vec<Function>) -> BTreeSet<String> {
    functions
        .into_iter()
        .map(|f| function_name(pipeline, f))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_pipeline_parse_accepts_mixed_function_names() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(()) -> (); specialize @B fn @bar(()) -> () {BODY}"
    );

    let parsed = pipeline.parse(&input).unwrap();
    assert_eq!(pipeline.stages().len(), 2);
    assert_eq!(
        parsed_names(&pipeline, parsed),
        BTreeSet::from(["bar".into(), "foo".into()])
    );
}

#[test]
fn test_pipeline_parse_uses_pipeline_global_table() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let input = format!("stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY}");

    let parsed = pipeline.parse(&input).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(function_name(&pipeline, parsed[0]), "foo");

    let mut external_globals: InternTable<String, GlobalSymbol> = InternTable::default();
    let bar = external_globals.intern("bar".to_string());
    let bar_raw: usize = bar.into();
    assert_eq!(bar_raw, 0, "external symbol table should remain untouched");
}

#[test]
fn test_stage_enum_pipeline_parse_uses_stage_symbol_mapping() {
    let mut pipeline: Pipeline<StageBucket> = Pipeline::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(()) -> (); specialize @B fn @bar(()) -> () {BODY}"
    );

    let parsed = pipeline.parse(&input).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(matches!(
        pipeline.stages(),
        [StageBucket::Parse(_), StageBucket::Lower(_)]
    ));
}

#[test]
fn test_stage_enum_pipeline_parse_rejects_unknown_stage_mapping() {
    let mut pipeline: Pipeline<StageBucket> = Pipeline::new();
    let err = pipeline.parse("stage @Z fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
}

#[test]
fn test_stage_enum_pipeline_parse_suggests_declared_name() {
    let mut pipeline: Pipeline<StageBucket> = Pipeline::new();
    let err = pipeline.parse("stage @C fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::UnknownStage);
    assert!(err.message.contains("@A"));
}

#[test]
fn test_stage_requires_semicolon() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let err = pipeline.parse("stage @A fn @foo(()) -> ()").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_specialize_requires_body() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let err = pipeline
        .parse("specialize @A fn @foo(()) -> ();")
        .unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_global_symbol_prefix_is_required() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let err = pipeline.parse("stage 1 fn @foo(()) -> ();").unwrap_err();
    assert_eq!(err.kind, crate::FunctionParseErrorKind::InvalidHeader);
}

#[test]
fn test_missing_stage_declaration_is_hard_error() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    pipeline
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();
    let input = format!("specialize @A fn @foo(()) -> () {BODY}");
    let err = pipeline.parse(&input).unwrap_err();
    assert_eq!(
        err.kind,
        crate::FunctionParseErrorKind::MissingStageDeclaration
    );
}

#[test]
fn test_comments_and_whitespace_are_accepted() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let input = format!(
        "/* stage declaration */ stage @A fn @foo(()) -> (); \
         // specialization body\n specialize @A fn @foo(()) -> () /* body */ {BODY}"
    );
    pipeline.parse(&input).unwrap();
}

#[test]
fn test_pipeline_roundtrip_print_parse_print() {
    let mut pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let stage_a = pipeline
        .add_stage()
        .stage(StageInfo::default())
        .name("A")
        .new();
    let function = pipeline.function().name("foo").new();
    let staged_function = pipeline
        .staged_function()
        .func(function)
        .stage(stage_a)
        .signature(unit_sig())
        .new()
        .unwrap();

    let stage = pipeline.stage_mut(stage_a).unwrap();
    let block = stage.block().new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::new(stage, region);
    stage
        .specialize()
        .f(staged_function)
        .signature(unit_sig())
        .body(body)
        .new()
        .unwrap();

    let rendered = function.sprint(&pipeline);

    let mut parsed_pipeline: Pipeline<StageInfo<FunctionBody>> = Pipeline::new();
    let parsed_functions = parsed_pipeline.parse(&rendered).unwrap();
    let parsed_function = parsed_functions
        .into_iter()
        .find(|id| function_name(&parsed_pipeline, *id) == "foo")
        .unwrap();

    let rendered_again = parsed_function.sprint(&parsed_pipeline);
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}

#[test]
fn test_pipeline_parse_uses_stage_language_dispatch() {
    let mut pipeline: Pipeline<MixedStage> = Pipeline::new();
    let input = format!(
        "stage @A fn @foo(()) -> (); \
         specialize @A fn @foo(()) -> () {BODY} \
         stage @B fn @bar(i32) -> i32; \
         specialize @B fn @bar(i32) -> i32 {BODY}"
    );

    let parsed = pipeline.parse(&input).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(matches!(
        pipeline.stages(),
        [MixedStage::StageA(_), MixedStage::StageB(_)]
    ));

    assert_eq!(
        parsed_names(&pipeline, parsed),
        BTreeSet::from(["bar".into(), "foo".into()])
    );

    let stage_b = pipeline
        .stages()
        .iter()
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
    assert_eq!(
        stage_b_sig,
        &Signature {
            params: vec![I32Type],
            ret: I32Type,
            constraints: ()
        }
    );
}
