//! Integration and unit tests for pretty printing.

use kirin_ir::{Block, Dialect, GlobalSymbol, InternTable, Pipeline};
use kirin_test_utils::*;
use prettyless::DocAllocator;

use crate::{ArenaDoc, Config, Document, FunctionPrintExt, PrettyPrint, PrettyPrintExt};

impl PrettyPrint for SimpleLanguage {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            SimpleLanguage::Add(lhs, rhs, _) => doc.text(format!("add {}, {}", *lhs, *rhs)),
            SimpleLanguage::Constant(value, _) => match value {
                Value::I64(v) => doc.text(format!("constant {}", v)),
                Value::F64(v) => doc.text(format!("constant {}", v)),
            },
            SimpleLanguage::Return(retval) => doc.text(format!("return {}", *retval)),
            SimpleLanguage::Function(region, _) => doc.print_region(region),
        }
    }
}

fn create_test_function() -> (
    kirin_ir::StageInfo<SimpleLanguage>,
    InternTable<String, GlobalSymbol>,
    kirin_ir::SpecializedFunction,
) {
    let mut gs: InternTable<String, GlobalSymbol> = InternTable::default();
    let test_func = gs.intern("test_func".to_string());
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();
    let staged_function = stage
        .staged_function()
        .name(test_func)
        .signature(kirin_ir::Signature {
            params: vec![Int],
            ret: Int,
            constraints: (),
        })
        .new()
        .unwrap();

    let a = SimpleLanguage::op_constant(&mut stage, 1.2);
    let b = SimpleLanguage::op_constant(&mut stage, 3.4);
    let c = SimpleLanguage::op_add(&mut stage, a.result, b.result);
    let block_arg_x = stage.block_argument(0);
    let d = SimpleLanguage::op_add(&mut stage, c.result, block_arg_x);
    let ret = SimpleLanguage::op_return(&mut stage, d.result);

    let block_a: Block = stage
        .block()
        .argument(Int)
        .argument_with_name("y", Float)
        .stmt(a)
        .stmt(b)
        .stmt(c)
        .stmt(d)
        .terminator(ret)
        .new();

    let ret = SimpleLanguage::op_return(&mut stage, block_arg_x);
    let block_b = stage.block().argument(Float).terminator(ret).new();

    let body = stage.region().add_block(block_a).add_block(block_b).new();
    let fdef = SimpleLanguage::op_function(&mut stage, body);
    let f = stage
        .specialize()
        .f(staged_function)
        .body(fdef)
        .new()
        .unwrap();

    (stage, gs, f)
}

include!("snapshot.rs");
include!("document.rs");
include!("pretty_print.rs");
include!("write.rs");
include!("global_symbol.rs");
include!("sprint_with_globals.rs");
include!("pipeline.rs");
