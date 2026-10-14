//! Round-trip coverage for the four body-kind discriminators in one pipeline.
//!
//! Every Kirin body kind is spelled with an explicit textual discriminator in
//! the default whole-field `{body}` position:
//!
//! ```text
//! fn @f(..) -> T cfg { ^entry(..) { .. } }   // CFG: keyword, then member blocks (untagged)
//! fn @f(..) -> T block ^body(..) { .. }      // Block
//! fn @f(..) -> T digraph ^g0(..) { .. }      // DiGraph
//! fn @f(..) -> T ungraph ^u0(..) { .. }      // UnGraph
//! ```
//!
//! `GraphFunctionLanguage` is the only test language with a callable variant
//! for all four, so it is where the full parse → emit → pretty-print → reparse
//! pipeline can be exercised on a single program.

use kirin_test_languages::GraphFunctionLanguage;
use kirin_test_utils::roundtrip;

/// One pipeline, four callables, one discriminator each — plus a CFG-bodied
/// `@main` whose `graph_eval` statement carries a *nested* DiGraph, so the
/// nested-body path is tagged and round-tripped too.
const ALL_BODY_KINDS: &str = r#"
stage @test fn @by_cfg(i64, i64) -> i64;
stage @test fn @by_block(i64, i64) -> i64;
stage @test fn @by_digraph(i64, i64) -> i64;
stage @test fn @by_ungraph(i64, i64) -> i64;
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @by_cfg(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %y: i64) {
    %s = add %x, %y -> i64;
    ret %s;
  }
  ^unused(%z: i64) {
    ret %z;
  }
}

specialize @test fn @by_block(i64, i64) -> i64 block ^body(%x: i64, %y: i64) {
  %s = sub %x, %y -> i64;
  ret %s;
}

specialize @test fn @by_digraph(i64, i64) -> i64 digraph ^g0(%x: i64, %y: i64) {
  %s = mul %x, %y -> i64;
  yield %s;
}

specialize @test fn @by_ungraph(i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  %t = mul %s, %s -> i64;
}

specialize @test fn @main(i64, i64) -> i64 cfg {
  ^entry(%x: i64, %y: i64) {
    %a = call.named @by_cfg(%x, %y) -> i64;
    %b = call.named @by_block(%x, %y) -> i64;
    %c = call.named @by_digraph(%x, %y) -> i64;
    %d = call.named @by_ungraph(%x, %y) -> i64;
    %e = graph_eval %a, %b digraph ^g1(%p: i64, %q: i64) {
      %n = add %p, %q -> i64;
      yield %n;
    } -> i64;
    %f = add %c, %d -> i64;
    %r = add %e, %f -> i64;
    ret %r;
  }
}
"#;

/// parse → emit → pretty-print → reparse → pretty-print, comparing both
/// renders. A missing or spurious discriminator on any of the four kinds
/// breaks either the first parse or the reparse.
#[test]
fn test_all_body_kinds_pipeline_roundtrip() {
    roundtrip::assert_pipeline_roundtrip::<GraphFunctionLanguage>(ALL_BODY_KINDS);
}

/// The printed form of each body kind carries its discriminator, and a CFG's
/// member blocks stay untagged.
#[test]
fn test_printed_body_kinds_carry_their_discriminators() {
    use kirin::prelude::*;

    let mut pipeline: Pipeline<StageInfo<GraphFunctionLanguage>> = Pipeline::new();
    pipeline
        .parse(ALL_BODY_KINDS)
        .expect("pipeline parse should succeed");
    let printed = pipeline.sprint();

    for expected in [
        "@by_cfg (i64, i64) -> i64 cfg {",
        "@by_block (i64, i64) -> i64 block ^body(%x: i64, %y: i64) {",
        "@by_digraph (i64, i64) -> i64 digraph ^g0(%x: i64, %y: i64) {",
        "@by_ungraph (i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {",
    ] {
        assert!(
            printed.contains(expected),
            "expected {expected:?} in printed pipeline:\n{printed}"
        );
    }

    // A CFG's member blocks are untagged — `cfg` names the body kind once for
    // the whole container.
    assert!(
        printed.contains("^entry(%x: i64, %y: i64) {"),
        "CFG member blocks should print untagged:\n{printed}"
    );
    assert!(
        !printed.contains("block ^entry"),
        "CFG member blocks must not be tagged:\n{printed}"
    );
}
