//! The dead-code cleanup the driver runs after each sweep, in `DiGraph`
//! bodies.

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_ir::walk::PostOrder;
use kirin_rewrite::{NativeRegistry, RewriteDriver, RewriteSummary, RuleSet};
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

/// `-(-x)` is `x`; the `neg`s are left to the cleanup.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// A one-function program whose body is a `DiGraph` on `%x: i64`.
fn graph_program(body: &str, yielded: &str) -> String {
    format!(
        "stage @g fn @f(i64) -> i64;\n\
         specialize @g fn @f(i64) -> i64 digraph ^graph(%x: i64) {{\n{body}\n  yield {yielded};\n}}\n"
    )
}

fn parse(input: &str) -> (Pipeline<StageInfo<L>>, CompileStage) {
    let mut pipeline: Pipeline<StageInfo<L>> = Pipeline::new();
    pipeline.parse(input).expect("the program parses");
    let stage = pipeline.stage_by_name("g").expect("the stage exists");
    (pipeline, stage)
}

fn printed(input: &str) -> String {
    parse(input).0.sprint()
}

/// Run `rules` over `input`, returning the printed result.
fn rewrite(rules: &RuleSet<L>, input: &str) -> (String, RewriteSummary) {
    let (mut pipeline, stage) = parse(input);
    let driver = RewriteDriver::new(rules);
    let summary = pipeline
        .run_pass(stage, |rewriter| driver.run::<PostOrder>(rewriter))
        .expect("the pass succeeds");
    (pipeline.sprint(), summary)
}

#[test]
fn dead_digraph_members_are_erased_from_the_sinks_back() {
    let no_rules = RuleSet::build(NativeRegistry::new(), |_| Vec::new()).unwrap();
    // `%u` is an unused sink. `%p` has two consumers, both dead, so it only
    // becomes dead once the second is erased. `%k` is a sink kept alive by
    // the yield.
    let input = graph_program(
        "  %u = add %x, %x -> i64;\n  \
           %p = neg %x -> i64;\n  \
           %e1 = add %p, %x -> i64;\n  \
           %e2 = add %p, %p -> i64;\n  \
           %k = neg %x -> i64;",
        "%k",
    );
    let (output, summary) = rewrite(&no_rules, &input);
    assert_eq!(
        output,
        printed(&graph_program("  %k = neg %x -> i64;", "%k"))
    );
    assert_eq!(summary.erased, 4);
}

#[test]
fn double_negation_is_removed_in_a_digraph() {
    let rules = RuleSet::build(NativeRegistry::new(), |stage| vec![double_neg(stage)]).unwrap();
    let input = graph_program("  %a = neg %x -> i64;\n  %b = neg %a -> i64;", "%b");
    let (output, summary) = rewrite(&rules, &input);
    assert_eq!(output, printed(&graph_program("", "%x")));
    assert_eq!(
        summary,
        RewriteSummary {
            sweeps: 2,
            applied: 1,
            erased: 2,
        }
    );
}
