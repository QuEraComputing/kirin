use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Call, Return};
use kirin_ir::{Dialect, Region, Signature};
use kirin_scf::StructuredControlFlow;

/// Test language for backward dataflow (liveness) analyses.
///
/// It composes a function body op with constants, arithmetic, comparisons,
/// unstructured control flow (`cf`), structured control flow (`scf`), calls,
/// and returns. Every variant is exercised by a liveness test (straight-line,
/// dead-pure ops, branch conditions, block-argument edges, joins, loops, calls,
/// multi-result, `scf.if`/`scf.for`).
///
/// # Why this is not a duplicate of an existing fixture
///
/// No other fixture here carries `Constant`, `Cmp`, or any `scf` op, and none
/// combines unstructured (`cf`) with structured (`scf`) control flow — that
/// split is deliberate elsewhere (e.g. `toy-lang` separates `cf` and `scf` by
/// stage). Liveness is the one consumer that must handle both control-flow
/// forms through a single generic solver, so its tests need them in one enum to
/// drive a single `LivenessOp` forwarding impl over a mixed dialect set.
/// Extending a shared fixture such as `ArithFunctionLanguage` (used by the
/// `cf`/`arith`/`namespace` roundtrip suites) to cover these would couple those
/// unrelated suites to four extra dialects. Hence a dedicated, feature-gated,
/// dev-only fixture instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum DataflowLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    Function {
        body: Region,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Structured(StructuredControlFlow<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
