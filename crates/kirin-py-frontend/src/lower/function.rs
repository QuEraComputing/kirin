use kirin::prelude::*;
use kirin_arith::ArithType;
use rustc_hash::FxHashMap;

use super::BlockBuf;
use super::stmt::lower_stmts;
use crate::ast::{FunctionDef, Module};
use crate::error::LowerError;
use crate::language::{PyLang, PyPipeline};
use crate::scope::Frame;
use crate::ty::map_type;

/// Lower a whole module to its `.kirin` IR string.
pub fn lower_module(module: &Module) -> Result<String, LowerError> {
    Ok(lower_to_pipeline(module)?.sprint())
}

/// Lower a whole module to a built [`PyPipeline`] (the in-memory IR).
///
/// Two-pass (mirrors `ParsePipelineText`): pass 1 declares every top-level
/// function as a staged function (so calls can resolve by name); pass 2 lowers
/// each body. Use this (rather than [`lower_module`]) when you want to *run* the
/// lowered IR, not just print it.
pub fn lower_to_pipeline(module: &Module) -> Result<PyPipeline, LowerError> {
    let mut pipeline = PyPipeline::new();
    let stage_id = pipeline
        .add_stage()
        .stage(StageInfo::default())
        .name("source")
        .new();

    // Pass 1: declarations.
    let mut staged: FxHashMap<String, StagedFunction> = FxHashMap::default();
    for func in &module.body {
        let handle = pipeline
            .function()
            .name(func.name.clone())
            .new()
            .map_err(|e| LowerError::Builder(format!("{e:?}")))?;
        let sf = pipeline
            .staged_function()
            .func(handle)
            .stage(stage_id)
            .signature(signature_of(func))
            .new()
            .map_err(|e| LowerError::Builder(format!("{e:?}")))?;
        staged.insert(func.name.clone(), sf);
    }

    // Pass 2: bodies.
    for func in &module.body {
        let sf = staged[&func.name];
        let result: Result<(), LowerError> = pipeline
            .stage_mut(stage_id)
            .expect("stage exists")
            .with_builder(|ctx| lower_function_def(ctx, func, sf));
        result?;
    }

    Ok(pipeline)
}

fn signature_of(func: &FunctionDef) -> Signature<ArithType> {
    let inputs = func
        .args
        .iter()
        .map(|a| map_type(a.annotation.as_ref()))
        .collect();
    let output = map_type(func.returns.as_ref());
    Signature::new(inputs, output, ())
}

fn lower_function_def(
    ctx: &mut BuilderStageInfo<PyLang>,
    func: &FunctionDef,
    sf: StagedFunction,
) -> Result<(), LowerError> {
    let mut frame = Frame::new();

    // Phase 1: build the entry block with arguments only, so parameters become
    // real BlockArgument SSAs (referenceable from nested if/for bodies).
    let mut builder = ctx.block().name("entry");
    for arg in &func.args {
        builder = builder
            .argument(map_type(arg.annotation.as_ref()))
            .arg_name(arg.name.clone());
    }
    let entry = builder.new();

    // Read back the real block-argument SSAs and bind parameter names.
    let arg_ssas: Vec<SSAValue> = ctx
        .block_arena()
        .get(entry)
        .expect("entry block exists")
        .arguments
        .iter()
        .map(|arg| SSAValue::from(Id::from(*arg)))
        .collect();
    for (arg, ssa) in func.args.iter().zip(arg_ssas) {
        frame.define(&arg.name, ssa);
    }

    // Phase 2: lower the body, then attach statements to the entry block.
    let mut buf = BlockBuf::new();
    lower_stmts(ctx, &func.body, &mut frame, &mut buf)?;
    let terminator = match buf.terminator {
        Some(term) => term,
        // Fell off the end without `return`: synthesize a void return.
        None => kirin_function::Return::<ArithType>::new(ctx, Vec::<SSAValue>::new()).id,
    };
    ctx.attach_statements_to_block(entry, &buf.stmts, Some(terminator));

    let body = ctx.region().add_block(entry).new();
    let fdef = kirin_function::Function::<ArithType>::new(ctx, body, signature_of(func));
    ctx.specialize()
        .staged_func(sf)
        .body(fdef.id)
        .new()
        .map_err(|e| LowerError::Builder(format!("{e:?}")))?;
    Ok(())
}
