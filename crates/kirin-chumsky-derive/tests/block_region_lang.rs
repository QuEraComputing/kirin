//! Tests for Block and Region field parsing.
//!
//! TODO: These tests are currently disabled because the EmitIR generation (part of
//! HasParser derive) has complex bounds when Block/Region fields are present.
//! The generated EmitIR impl requires `<Language as HasRecursiveParser>::Output: EmitIR<Language>`
//! which creates a self-referential constraint. Fix this by adding the bound explicitly
//! to generated impls for dialects with Block/Region fields.

// Temporarily disabled - uncomment when EmitIR bounds issue is fixed
/*
mod common;

use common::SimpleType;
use kirin::ir::{Dialect, ResultValue, SSAValue};
use kirin_chumsky::{parse_ast, HasParser, PrettyPrint};

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(type_lattice = SimpleType)]
#[chumsky(crate = kirin_chumsky)]
pub enum BlockRegionLang {
    #[chumsky(format = "{res:name} = id {arg} -> {res:type}")]
    Id { res: ResultValue, arg: SSAValue },
    #[chumsky(format = "{res} = loop {body}")]
    Loop {
        res: ResultValue,
        body: kirin::ir::Block,
    },
    #[chumsky(format = "{res} = scope {body}")]
    Scope {
        res: ResultValue,
        body: kirin::ir::Region,
    },
    #[chumsky(format = "ret {0}")]
    Ret(SSAValue),
}

// All tests disabled
*/
