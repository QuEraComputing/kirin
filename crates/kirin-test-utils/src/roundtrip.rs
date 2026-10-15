use std::fmt::Display;

use kirin_chumsky::{ParsePipelineText, ParseStatementTextExt, PrettyPrint};
use kirin_ir::{BuilderSSAKind, Dialect, Pipeline, StageInfo, Statement};
use kirin_prettyless::{Config, Document, PipelinePrintExt, RenderDispatch};

/// Parse a single statement with pre-registered operands.
pub fn emit_statement<L>(input: &str, operands: &[(&str, L::Type)]) -> (StageInfo<L>, Statement)
where
    L: Dialect,
    StageInfo<L>: ParseStatementTextExt<L>,
    L::Type: Clone,
{
    let mut stage: StageInfo<L> = StageInfo::default();

    stage.with_builder(|b| {
        // Create a dummy block to serve as the owner of operand SSAs
        let dummy_block = b.block().new();
        for (idx, (name, ty)) in operands.iter().enumerate() {
            b.ssa()
                .name((*name).to_string())
                .ty(ty.clone())
                .kind(BuilderSSAKind::BlockArgument(dummy_block, idx))
                .new();
        }
    });

    let statement = stage.parse_statement(input).expect("parse should succeed");
    (stage, statement)
}

/// Pretty-print a statement to a string.
pub fn render_statement<L>(stage: &StageInfo<L>, statement: Statement) -> String
where
    L: Dialect + PrettyPrint,
    L::Type: Display,
{
    let doc = Document::new(Config::default(), stage);
    let arena_doc = doc.print_statement(&statement);
    let mut output = String::new();
    arena_doc
        .render_fmt(80, &mut output)
        .expect("render should succeed");
    output
}

/// Assert that a statement roundtrips: parse → render → reparse → render → compare.
pub fn assert_statement_roundtrip<L>(input: &str, operands: &[(&str, L::Type)])
where
    L: Dialect + PrettyPrint + RoundtripSerde,
    StageInfo<L>: ParseStatementTextExt<L>,
    L::Type: Clone + Display + RoundtripSerde,
{
    let (stage, statement) = emit_statement::<L>(input, operands);
    let rendered = render_statement::<L>(&stage, statement);
    #[cfg(feature = "serde")]
    {
        let restored = json_roundtrip(&stage);
        assert_eq!(rendered, render_statement::<L>(&restored, statement));
        kirin_ir::verify_derived(&restored).expect("restored metadata is valid");
    }

    let (reparsed_stage, reparsed_statement) = emit_statement::<L>(rendered.trim(), operands);
    let rendered_again = render_statement::<L>(&reparsed_stage, reparsed_statement);

    assert_eq!(rendered.trim(), rendered_again.trim());
}

/// Assert that a pipeline roundtrips: parse → render → reparse → render → compare.
pub fn assert_pipeline_roundtrip<L>(input: &str)
where
    Pipeline<StageInfo<L>>: ParsePipelineText,
    StageInfo<L>: RenderDispatch,
    L: Dialect + RoundtripSerde,
    L::Type: RoundtripSerde,
{
    let mut parsed_pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    let parsed_functions = parsed_pipeline
        .parse(input)
        .expect("pipeline parse should succeed");
    assert!(
        !parsed_functions.is_empty(),
        "expected at least one parsed function"
    );
    let rendered = parsed_pipeline.sprint();
    #[cfg(feature = "serde")]
    {
        let restored = json_roundtrip(&parsed_pipeline);
        assert_eq!(rendered, restored.sprint());
        for stage in restored.stages() {
            kirin_ir::verify_derived(stage).expect("restored metadata is valid");
        }
        for function in restored.function_arena().iter() {
            if let Some(name) = function.name() {
                assert_eq!(restored.function_by_name(name), Some(function.id()));
            }
        }
    }

    let mut reparsed_pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    let reparsed_functions = reparsed_pipeline
        .parse(&rendered)
        .expect("rendered pipeline should reparse");
    assert_eq!(
        reparsed_functions.len(),
        parsed_functions.len(),
        "expected same number of parsed functions"
    );
    let rendered_again = reparsed_pipeline.sprint();
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}

/// Serde bound enabled only for JSON qualification builds.
#[cfg(feature = "serde")]
pub trait RoundtripSerde: serde::Serialize + serde::de::DeserializeOwned {}
#[cfg(feature = "serde")]
impl<T: serde::Serialize + serde::de::DeserializeOwned> RoundtripSerde for T {}
#[cfg(not(feature = "serde"))]
pub trait RoundtripSerde {}
#[cfg(not(feature = "serde"))]
impl<T> RoundtripSerde for T {}

/// Preserve all serialized IR fields, including IDs, types, attributes and links.
#[cfg(feature = "serde")]
pub fn json_roundtrip<T: RoundtripSerde>(value: &T) -> T {
    let json = serde_json::to_string(value).expect("IR serializes to JSON");
    let restored = serde_json::from_str(&json).expect("JSON restores IR");
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::to_value(&restored).unwrap(),
        "JSON round trip must preserve every encoded field"
    );
    restored
}
