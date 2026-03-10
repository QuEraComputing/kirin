use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_function::{Bind, Call, Return};
use kirin_scf::Yield;

/// Source-stage language: structured control flow + lexical lambdas.
///
/// Block/Region-containing types (Function, Lambda, If, For) are inlined
/// because the recursive AST types overflow trait resolution (E0275) when
/// composed via `#[wraps]` + `HasParser`. Constant is inlined because its
/// multi-param `EmitIR` bound is not yet resolved by the `ParseDispatch`
/// macro. All other variants use `#[wraps]` delegation to built-in dialect
/// types.
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

    #[wraps]
    Yield(Yield<ArithType>),

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
/// Function body and Constant are inlined; all other variants use `#[wraps]`
/// delegation to built-in dialect types.
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
