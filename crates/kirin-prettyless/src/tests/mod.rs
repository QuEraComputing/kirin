//! Integration and unit tests for pretty printing.

use kirin_ir::{Block, BuilderStageInfo, Dialect, GlobalSymbol, InternTable, Pipeline, StageInfo};
use kirin_test_languages::*;
use prettyless::DocAllocator;

use crate::{ArenaDoc, Config, Document, PrettyPrint, PrettyPrintExt, PrintExt};

impl PrettyPrint for SimpleLanguage {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            SimpleLanguage::Add(lhs, rhs, _res) => {
                doc.text("add ") + lhs.pretty_print(doc) + doc.text(", ") + rhs.pretty_print(doc)
            }
            SimpleLanguage::Constant(value, _res) => doc.text(format!("constant {}", value)),
            SimpleLanguage::Return(retval) => doc.text("return ") + retval.pretty_print(doc),
            SimpleLanguage::Function(region, _) => doc.print_region(region),
        }
    }
}

fn create_test_function() -> (
    StageInfo<SimpleLanguage>,
    InternTable<String, GlobalSymbol>,
    kirin_ir::SpecializedFunction,
) {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("test_func".to_string());
    let mut stage: BuilderStageInfo<SimpleLanguage> = BuilderStageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature::new(
            vec![SimpleType::I64],
            SimpleType::I64,
            (),
        ))
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1.2);
    let b = SimpleLanguage::op_constant(&mut stage, 3.4);
    let c = SimpleLanguage::op_add(&mut stage, a.result, b.result);
    let block_arg_x = stage.block_argument().index(0);
    let d = SimpleLanguage::op_add(&mut stage, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut stage, d.result);

    let block_a: Block = stage
        .block()
        .argument(SimpleType::I64)
        .argument(SimpleType::F64)
        .arg_name("y")
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .stmt(d)
        .terminator(ret)
        .new();

    let ret = SimpleLanguage::op_return(&mut stage, block_arg_x);
    let block_b = stage
        .block()
        .argument(SimpleType::F64)
        .terminator(ret)
        .new();

    let body = stage.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let f = stage
        .specialize()
        .staged_func(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    (stage.finalize().unwrap(), gs, f)
}

include!("snapshot.rs");
include!("document.rs");
include!("pretty_print.rs");
include!("write.rs");
include!("global_symbol.rs");
include!("sprint_with_globals.rs");
include!("pipeline.rs");
include!("impls.rs");
include!("edge_cases.rs");
include!("graph_snapshot.rs");
