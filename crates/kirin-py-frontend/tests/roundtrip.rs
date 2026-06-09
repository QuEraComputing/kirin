//! The lowered `.kirin` must be valid, re-parseable Kirin: for every fixture,
//! print → parse → print is stable.

mod common;

use common::all_modules;
use kirin::prelude::*;
use kirin_py_frontend::{PyPipeline, lower_module};

#[test]
fn lowered_ir_roundtrips_through_parser() {
    for module in all_modules() {
        let printed = lower_module(&module).unwrap();
        let mut pipeline = PyPipeline::new();
        ParsePipelineText::parse(&mut pipeline, &printed)
            .expect("lowered .kirin should parse back");
        let reprinted = pipeline.sprint();
        assert_eq!(printed.trim_end(), reprinted.trim_end());
    }
}
