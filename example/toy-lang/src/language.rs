use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_function::{Bind, Call, Return};

/// Source-stage language: structured control flow + lexical lambdas.
///
/// Block/Region-containing types (If, For, Lambda) are inlined to avoid
/// E0275 trait recursion overflow with `#[wraps]` + `HasParser`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[chumsky(format = "{res:name} = {.lambda} {name} captures({captures}) {body} -> {res:type}")]
    Lambda {
        name: Symbol,
        captures: Vec<SSAValue>,
        body: Region,
        res: ResultValue,
    },

    #[chumsky(format = "{.if} {condition} then {then_body} else {else_body}")]
    If {
        condition: SSAValue,
        then_body: Block,
        else_body: Block,
    },

    #[chumsky(format = "{.for} {induction_var} in {start}..{end} step {step} do {body}")]
    For {
        induction_var: SSAValue,
        start: SSAValue,
        end: SSAValue,
        step: SSAValue,
        body: Block,
    },

    #[kirin(terminator)]
    #[chumsky(format = "{.yield} {value}")]
    Yield { value: SSAValue },

    #[kirin(constant, pure)]
    #[chumsky(format = "{result:name} = {.constant} {value} -> {result:type}")]
    Constant {
        #[kirin(into)]
        value: ArithValue,
        #[kirin(type = value.type_of())]
        result: ResultValue,
    },

    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

/// Lowered-stage language: unstructured CF + lifted functions.
///
/// Function body is inlined (Region field, E0275 workaround).
/// All other variants use `#[wraps]` delegation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum LowLevel {
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[kirin(constant, pure)]
    #[chumsky(format = "{result:name} = {.constant} {value} -> {result:type}")]
    Constant {
        #[kirin(into)]
        value: ArithValue,
        #[kirin(type = value.type_of())]
        result: ResultValue,
    },

    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
    #[wraps]
    Bind(Bind<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
