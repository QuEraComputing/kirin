# Part I - Syntax

> Part of the [Rust Interpreter Formalism](index.md).

This syntax part is intentionally API-first (actual Rust types and files), with
just enough formal shorthand to compose with Parts II-IV.

## Reading Recipe

- **Formal read:** Treat this as the grammar-level domain of `s` (statement), owners (`Region`/`Block`), and SSA carriers consumed by the judgment.
- **API read:** Verify mappings in `crates/kirin-ir/src/{pipeline.rs,stage/info.rs,language.rs,node/{function/*,region.rs,block.rs,stmt.rs,ssa.rs},product.rs}` and stage/language composition in `example/toy-lang/src/{language.rs,stage.rs}`.

The formal names below map directly to concrete Rust IR/runtime types.

| Formal name | Rust type / trait | Code |
| --- | --- | --- |
| `Program` | `Pipeline<S>` | [`crates/kirin-ir/src/pipeline.rs`](../../../crates/kirin-ir/src/pipeline.rs) |
| `Stage` | `StageInfo<L>` plus stage enum implementing `StageMeta` | [`crates/kirin-ir/src/stage/info.rs`](../../../crates/kirin-ir/src/stage/info.rs), [`crates/kirin-ir/src/stage/meta.rs`](../../../crates/kirin-ir/src/stage/meta.rs), [`example/toy-lang/src/stage.rs`](../../../example/toy-lang/src/stage.rs) |
| `Language` | dialect enum `L: Dialect` (often wrapper enum with `#[wraps]`) | [`crates/kirin-ir/src/language.rs`](../../../crates/kirin-ir/src/language.rs), [`example/toy-lang/src/language.rs`](../../../example/toy-lang/src/language.rs) |
| `Function` | `FunctionInfo` / `Function` | [`crates/kirin-ir/src/node/function/generic.rs`](../../../crates/kirin-ir/src/node/function/generic.rs) |
| `StagedFunction` | `StagedFunctionInfo` / `StagedFunction` | [`crates/kirin-ir/src/node/function/staged.rs`](../../../crates/kirin-ir/src/node/function/staged.rs) |
| `SpecializedFunction` | `SpecializedFunctionInfo` / `SpecializedFunction` | [`crates/kirin-ir/src/node/function/specialized.rs`](../../../crates/kirin-ir/src/node/function/specialized.rs) |
| `Region` | `RegionInfo` / `Region` | [`crates/kirin-ir/src/node/region.rs`](../../../crates/kirin-ir/src/node/region.rs) |
| `Block` | `BlockInfo` / `Block` / `Successor` | [`crates/kirin-ir/src/node/block.rs`](../../../crates/kirin-ir/src/node/block.rs) |
| `Statement` | `StatementInfo` / `Statement` | [`crates/kirin-ir/src/node/stmt.rs`](../../../crates/kirin-ir/src/node/stmt.rs) |
| `SSAValue` | `SSAValue`, `ResultValue`, `BlockArgument` | [`crates/kirin-ir/src/node/ssa.rs`](../../../crates/kirin-ir/src/node/ssa.rs) |
| Multi-value packet | `Product<T>` | [`crates/kirin-ir/src/product.rs`](../../../crates/kirin-ir/src/product.rs) |

## I.2 IR shape (interpreter-relevant)

Kirin syntax for interpreter purposes is SSA IR over staged pipelines:

- `Pipeline<S>` is the immutable top-level program container.
- `StageInfo<L>` is per-stage storage for one language `L`.
- `L: Dialect` is the stage language (often an enum wrapping multiple dialects).
- `Function -> StagedFunction -> SpecializedFunction` is the callable hierarchy.
- `Region -> Block -> Statement` is executable structure.
- `SSAValue` connects operands/results/block arguments across statements.

The interpreter executes this graph-like SSA structure, not an expression tree.

## I.3 Minimal grammar skeleton

```
Program        ::= Pipeline(Stage*)
Stage          ::= StageInfo(Language)
Language       ::= DialectEnumVariant*

Function       ::= FunctionInfo + staged variants
StagedFunction ::= stage-specific callable variant
Specialized    ::= concrete specialization with body Statement

Statement      ::= dialect definition + operands + results + nested blocks/regions/successors
Region         ::= Block*
Block          ::= BlockArgument* ; Statement* ; optional terminator cache
SSAValue       ::= ResultValue | BlockArgument | Port
```

`Statement.definition` is the dialect value dispatchable through
`InterpDispatch -> Interpretable`.

## I.4 Dialect composition surface

A stage language composes multiple dialect families as wrapped variants (for
example function + scf + arith + cmp + cf depending on stage). The current
composition pattern is illustrated in:

- [`example/toy-lang/src/language.rs`](../../../example/toy-lang/src/language.rs)

`#[derive(Interpretable)]` delegates variant dispatch to the wrapped dialect
implementation.

## I.5 Dispatch-critical syntax classes

For operational semantics, statement forms divide into:

1. **Atomic forms**: produce values, then `Effect::Next`.
2. **CFG transfer forms**: produce `Jump` or `Branch`.
3. **Call/return forms**: produce `Call` / `Return`.
4. **Structured scope forms**: produce `Enter`, `EnterAny`, or `Yield`.

This partition is semantic, but driven by statement definition shape in dialect
impls.

## I.6 Structural well-formedness assumptions

Interpreter correctness relies on:

1. Stage and statement references are valid in `Pipeline`.
2. Block argument arities match incoming edge products.
3. Call argument/result arities match target signatures.
4. Statement ordering and successor metadata are coherent.
5. Terminator conventions are respected by builders/dialects.

Some checks happen during IR building/finalization; many are runtime checked as
typed interpreter errors.

## I.7 Syntax-to-semantics bridge

Given statement `s`:

1. fetch definition at `(stage, statement)`
2. dispatch by stage language via `InterpDispatch`
3. call dialect `interpret(&mut I)`
4. obtain `Effect`
5. engine applies global control semantics

This is the core decoupling: syntax chooses local statement meaning; engine mode
concrete vs abstract chooses global traversal/fixpoint behavior.
