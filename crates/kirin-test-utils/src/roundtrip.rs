use std::fmt::Display;

use kirin_chumsky::{EmitIR, HasParser, ParsePipelineText, PrettyPrint, parse};
use kirin_ir::{Dialect, GetInfo, Pipeline, SSAKind, StageInfo, Statement};
use kirin_prettyless::{Config, Document, FunctionPrintExt, RenderStage};

/// Parse a single statement with pre-registered operands.
pub fn emit_statement<'src, L>(
    input: &'src str,
    operands: &[(&str, L::Type)],
) -> (StageInfo<L>, Statement)
where
    L: Dialect + HasParser<'src, 'src>,
    L::Output: EmitIR<L, Output = Statement>,
    L::Type: Clone,
{
    let mut stage: StageInfo<L> = StageInfo::default();

    for (name, ty) in operands {
        stage
            .ssa()
            .name((*name).to_string())
            .ty(ty.clone())
            .kind(SSAKind::Test)
            .new();
    }

    let statement = parse::<L>(input, &mut stage).expect("parse should succeed");
    (stage, statement)
}

/// Pretty-print a statement to a string.
pub fn render_statement<L>(stage: &StageInfo<L>, statement: Statement) -> String
where
    L: Dialect + PrettyPrint,
    L::Type: Display,
{
    let dialect = statement
        .get_info(stage)
        .expect("statement should exist")
        .definition();

    let doc = Document::new(Config::default(), stage);
    let mut output = String::new();
    dialect
        .pretty_print(&doc)
        .render_fmt(80, &mut output)
        .expect("render should succeed");
    output
}

/// Assert that a statement roundtrips: parse → render → reparse → render → compare.
pub fn assert_statement_roundtrip<L>(input: &str, operands: &[(&str, L::Type)])
where
    L: Dialect + PrettyPrint + for<'a> HasParser<'a, 'a>,
    for<'a> <L as HasParser<'a, 'a>>::Output: EmitIR<L, Output = Statement>,
    L::Type: Clone + Display,
{
    let (stage, statement) = emit_statement::<L>(input, operands);
    let rendered = render_statement::<L>(&stage, statement);

    let (reparsed_stage, reparsed_statement) = emit_statement::<L>(rendered.trim(), operands);
    let rendered_again = render_statement::<L>(&reparsed_stage, reparsed_statement);

    assert_eq!(rendered.trim(), rendered_again.trim());
}

/// Assert that a pipeline roundtrips: parse → render → reparse → render → compare.
pub fn assert_pipeline_roundtrip<L>(input: &str)
where
    Pipeline<StageInfo<L>>: ParsePipelineText,
    StageInfo<L>: RenderStage,
    L: Dialect,
{
    let mut parsed_pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    let parsed_functions = parsed_pipeline
        .parse(input)
        .expect("pipeline parse should succeed");
    assert!(
        !parsed_functions.is_empty(),
        "expected at least one parsed function"
    );
    let rendered = parsed_functions
        .iter()
        .map(|function| function.sprint(&parsed_pipeline))
        .collect::<Vec<_>>()
        .join("\n");

    let mut reparsed_pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    let reparsed_functions = reparsed_pipeline
        .parse(&rendered)
        .expect("rendered pipeline should reparse");
    assert_eq!(
        reparsed_functions.len(),
        parsed_functions.len(),
        "expected same number of parsed functions"
    );
    let rendered_again = reparsed_functions
        .iter()
        .map(|function| function.sprint(&reparsed_pipeline))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.trim_end(), rendered_again.trim_end());
}
