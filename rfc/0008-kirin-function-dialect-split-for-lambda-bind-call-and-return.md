+++
rfc = "0008"
title = "Kirin function dialect split for lambda bind call and return"
status = "Draft"
agents = ["codex"]
authors = ["Roger-luo <code@rogerluo.dev>"]
created = "2026-02-11T04:12:13.82714Z"
last_updated = "2026-02-11T05:12:30.929235Z"
dependencies = ["0001"]
+++

# RFC 0008: Kirin function dialect split for lambda bind call and return

## Summary

Define a dialect split inside `kirin-function` for closure and call semantics:
`lambda`, `callable` (`bind`, `apply`), `call`, and `ret`.
This gives a high-level IR shape that stays close to AST syntax while keeping
lowered function specialization/backedge logic in `kirin-ir`. The RFC also
standardizes the mapping from high-level lambda statements to hoisted staged
functions used by closure-lifting transforms. Operation contracts are defined
generically so they can be reused across type domains, and the RFC includes
both high-level and lowered text forms as roundtrip verification targets.
Closure lifting semantics, specialization integration, and backedge expectations
are part of this same RFC.

## Motivation

- Problem: current `kirin-function` only provides a minimal region-container
  statement and does not define closure/call/return operations in a cohesive
  way. This creates ambiguity in how high-level AST-like function constructs map
  into IR before closure lifting.
- Why now: closure support work needs a stable high-level statement vocabulary
  before implementing lowering.
- Stakeholders:
  - `kirin-function` maintainers
  - `kirin-ir` and pass authors implementing closure lifting
  - `kirin-chumsky` / `kirin-prettyless` maintainers
  - users writing high-level dialects that need lambda and function calls

## Goals

- Define statement-level operations in `kirin-function` for:
  - lambda expression construction
  - closure binding
  - callable invocation and direct call
  - return terminator
- Keep operation contracts generic over captures, arguments, and result types so
  they are reusable across frontends and lowering passes.
- Preserve specialization/backedge semantics by lowering closures into normal
  staged functions with explicit environment parameters.
- Keep operations modular by putting them in separate dialect modules.
- Preserve compatibility with RFC 0001 top-level `stage`/`specialize` syntax.
- Provide a precise high-level-to-lowered mapping contract for closure lifting.

## Non-goals

- Adding nested `stage`/`specialize` declarations inside block bodies in
  canonical text format.
- Defining cross-stage closure materialization in v1.
- Defining full escape analysis or closure allocation optimization in v1.
- Replacing existing `kirin-cf` control-flow operations.

## Guide-level Explanation

At high level, users can author lambda statements and call them, while the IR
still keeps explicit function declarations at top level.

Canonical high-level text format (roundtrip target):

```text
stage @A fn @main(number) -> number;
specialize @A fn @main(number) -> number {
  ^bb0(%x: number) {
    %f = lambda @closure(number) -> number when captures(%x) {
      ^bb1(%y: number) {
        %x_1 = %capture.x;
        %r = add %x_1 %y;
        ret %r;
      }
    }
    %r = apply %f(%x);
    ret %r;
  }
}
```

This RFC defines the statement vocabulary used by that syntax. A later lowering
pass hoists `lambda` bodies into top-level staged functions and
rewrites value construction to `bind`.

## Reference-level Explanation

### API and syntax changes

`kirin-function` is split into dialect modules:

- `dialects/lambda.rs`:
  - high-level lambda statement with explicit captures and body region.
- `dialects/callable.rs`:
  - `bind` for constructing callable values.
  - `apply` for invoking callable values.
- `dialects/call.rs`:
  - direct function call operation (`call`).
- `dialects/ret.rs`:
  - return terminator (`ret`).
- `dialects/mod.rs`:
  - wrapper enums / re-exports for composition.

### Generic reusable operation contracts

For generic capture types `C...`, runtime argument types `P...`, and result type
`R`, operation contracts are:

- `lambda`: `(captures: C..., params: P..., body: (C..., P...) -> R) ->
  Callable<P... -> R>`
- `bind`: `(target: @f(C..., P...) -> R, captures: C...) ->
  Callable<P... -> R>`
- `apply`: `(callee: Callable<P... -> R>, args: P...) -> R`
- `call`: `(target: @f(P...) -> R, args: P...) -> R`
- `ret`: `(value: R) -> terminator`

### Closure-lifting integration with function model

This RFC also specifies the closure-lifting contract used by `kirin-ir`:

- every lambda body is hoisted to a deterministic staged function symbol
  (for example `@<owner>$closure<idx>`) in v1.
- lifted function signatures prepend an explicit environment parameter:
  `(EnvTy, Arg0, ..., ArgN) -> Ret`.
- specialization remains function-centric through existing
  `StageInfo::specialize` + `SignatureSemantics`; no new specialization
  entrypoint is introduced for closures.
- backedge behavior remains function-centric; closure calls with known targets
  contribute backedges to lifted staged/specialized functions.
- captured values are by-value and immutable in v1.

Illustrative statement definitions:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[wraps]
#[kirin(fn, type = T)]
pub enum Callable<T: CompileTimeValue + Default> {
    Bind(Bind<T>),   // prints as bind
    Apply(Apply<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = bind {target} captures({captures}) -> {res:type}")]
pub struct Bind<T: CompileTimeValue + Default> {
    target: GlobalSymbol,
    captures: Vec<SSAValue>,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = lambda {name}({params}) -> {ret} when captures({captures}) {body}")]
pub struct Lambda<T: CompileTimeValue + Default> {
    name: Option<GlobalSymbol>,
    captures: Vec<SSAValue>,
    body: Region,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = T)]
#[chumsky(format = "{res:name} = call {callee}({args}) -> {res:type}")]
pub struct Call<T: CompileTimeValue + Default> {
    callee: GlobalSymbol,
    args: Vec<SSAValue>,
    #[kirin(type = T::default())]
    res: ResultValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(terminator, fn, type = T)]
#[chumsky(format = "ret {value}")]
pub struct Return<T: CompileTimeValue + Default> {
    value: SSAValue,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}
```

Notes:

- concrete `#[chumsky(format = ...)]` strings are illustrative; final grammar
  may need minor parser-specific adjustments for list fields.

### Proposed text format (roundtrip verification targets)

High-level lambda form:

- Use the canonical high-level snippet in Guide-level Explanation as the
  high-level roundtrip target.

Lowered callable form:

```text
stage @A fn @main(number) -> number;
stage @A fn @closure(number, number) -> number;

specialize @A fn @closure(number, number) -> number {
  ^bb0(%capt0: number, %arg0: number) {
    %sum = add %capt0 %arg0;
    ret %sum;
  }
}

specialize @A fn @main(number) -> number {
  ^bb0(%x: number) {
    %f = bind @closure captures(%x) -> callable(number -> number);
    %r_apply = apply %f(%x);
    %r_call = call @closure(%x, %x) -> number;
    ret %r_apply;
  }
}
```

Roundtrip expectations for later verification:

- canonical high-level snippet in Guide-level Explanation supports
  `print -> parse -> print` with only whitespace/layout normalization.
- lowered `bind` snippet supports `print -> parse -> print` with only
  whitespace/layout normalization.

### Semantics and invariants

- Definitions are parametric over capture tuple, runtime argument tuple, and
  result type; no operation semantics are tied to `number`.
- `lambda` is a high-level statement that produces a callable value and may
  capture SSA values from the enclosing scope.
- `bind` produces callable values by pairing a target function identity
  with capture values.
- `apply` invokes callable values; target may be unknown at parse time.
- `call` invokes a known function symbol directly.
- no semantic aliases are defined in v1; each operation has a distinct role.
- `ret` is the canonical function-level return terminator in `kirin-function`.
- Canonical top-level declarations remain `stage` / `specialize` (RFC 0001).
  This RFC does not permit declaration statements inside blocks.
- Lowering contract:
  - `lambda` lowers to hoisted staged function + bind statement
  - hoisted signatures are `(EnvTy, Args...) -> Ret`
  - captures become explicit env representation in lowered form
  - `apply` may lower to direct `call` when target is proven unique
  - specialization and invalidation reuse existing function-level machinery

### Crate impact matrix

| crate | impact | tests to update |
| --- | --- | --- |
| `kirin-function` | add dialect modules `lambda`, `callable`, `call`, `ret` and wrapper exports | unit tests for each operation + composition tests |
| `kirin-chumsky` | parse support for the new statement forms | parse/emit roundtrip tests for high-level lambda and lowered `bind` form |
| `kirin-prettyless` | pretty-print support and snapshots for each dialect op | snapshot tests for canonical print style |
| `kirin-ir` | callable/type interop touchpoints used by lowering passes and function model APIs (`StageInfo::specialize`, backedges) | integration tests with lifted-closure specialization/backedge expectations |
| `kirin-test-utils` | reusable fixtures for lambda/callable/call/ret tests | add helpers used by multiple crates |

## Drawbacks

- More dialect surface area in `kirin-function`.
- Potential naming overlap between `apply` and direct `call` requires clear
  teaching and docs.
- Environment typing can increase specialization fan-out for heavily-captured
  callsites.

## Rationale and Alternatives

### Proposed approach rationale

- Splitting operations by concern keeps modules small and composable, matching
  project guidance to avoid giant modules and giant imported name sets.
- `lambda` as separate dialect cleanly models high-level syntax without forcing
  low-level binding details into the same operation.
- Keeping `call` and `ret` in `kirin-function` provides a coherent function
  authoring surface independent of `kirin-cf`.

### Alternative A

- Description: keep a single monolithic `kirin-function` dialect containing all
  function-related operations.
- Pros:
  - fewer module files
  - simple re-export story
- Cons:
  - rapidly growing operation set in one module
  - weaker separation between high-level and low-level statements
- Reason not chosen: reduced maintainability and poorer separation of concerns.

### Alternative B

- Description: define only `lambda` and rely on other crates (`kirin-cf`) for
  `call`/`ret`.
- Pros:
  - minimal additions in `kirin-function`
- Cons:
  - fragmented user story for function authoring
  - unclear ownership for closure-specific call behavior (`apply`)
- Reason not chosen: function-authoring primitives should live together in
  `kirin-function`.

## Prior Art

- MLIR separation between region-level high-level constructs and lower-level
  call/cfg operations.
- Julia high-level anonymous function syntax lowered to internal callable forms.
- Existing Kirin RFC 0001 decision to keep function declarations explicit and
  top-level.

## Backward Compatibility and Migration

- Breaking changes: none for existing RFC 0001 declaration syntax.
- Migration steps:
  1. Introduce new dialect modules and wrapper exports in `kirin-function`.
  2. Add parser/printer support and examples for `lambda`, `bind`,
     `apply`, `call`, `ret`.
  3. Migrate existing function examples to use `ret` from `kirin-function` where
     appropriate.
- Compatibility strategy:
  - keep legacy examples valid where possible
  - use `bind` as the single callable-construction spelling in v1

## How to Teach This

- Teach operation families:
  - `lambda` creates callable syntax-level closures
  - `bind` constructs callable objects
  - `apply` calls callable objects, `call` calls known symbols
  - `ret` terminates function bodies
- Update:
  - `crates/kirin-function/README` and crate docs
  - `design/function.md` and closure examples
  - parser/printer documentation examples with your `@main(number)` snippet

## Reference Implementation Plan

1. Restructure `kirin-function` into dialect modules under
   `crates/kirin-function/src/dialects/` using `mod.rs`.
2. Add statement definitions for `lambda`, `bind`, `apply`, `call`, and
   `ret`.
3. Implement parser/printer support and snapshots in affected crates.
4. Add shared test fixtures to `kirin-test-utils`.
5. Wire closure-lifting pass against these new statement definitions and verify
   specialization/backedge behavior against existing function-model APIs.

### Acceptance Criteria

- [ ] `kirin-function` exposes separate dialect modules for lambda, callable,
      call, and return operations.
- [ ] High-level lambda and lowered callable examples in this RFC parse and
      print in `kirin-chumsky` / `kirin-prettyless` coverage (modulo
      whitespace normalization).
- [ ] `bind` is available as the callable-construction operation in v1.
- [ ] `ret` and direct `call` are available from `kirin-function` as separate
      dialect operations.
- [ ] No duplicated callable-construction semantics remain in v1.
- [ ] Lifted closure functions specialize through existing
      `StageInfo::specialize` behavior with no closure-specific specialization
      entrypoint.
- [ ] Closure calls with known targets integrate with existing function-level
      backedge/invalidation behavior.

### Tracking Plan

- Tracking issue: TBD
- Implementation PRs: TBD
- Follow-up tasks:
  - evaluate whether `apply` should eventually carry effect/speculation
    attributes

## Unresolved Questions

- What is the final textual representation for callable and lambda type fields
  in `#[chumsky(format = ...)]` without introducing parser ambiguity?
- Should direct `call` refer to `GlobalSymbol`, `StagedFunction`, or both at
  different pipeline phases?

## Future Possibilities

- Add nested-function sugar with explicit local symbols lowered through the
  closure-lifting contract defined in this RFC.
- Add devirtualization transforms that rewrite `apply` to `call`.
- Add richer capture modes (by-ref/mutable) if needed by target languages.
- Add cross-stage closure lifting policy once same-stage v1 behavior is stable.

## Revision Log

| Date | Change |
| --- | --- |
| 2026-02-11T04:12:13.82714Z | RFC created from template |
| 2026-02-11 | Replaced template placeholders with concrete dialect split for `lambda`, `bind`, `call`, and `ret` |
| 2026-02-11 | Removed `Function*` prefixes from illustrative statement type names (`Callable`, `Call`, `Return`) |
| 2026-02-11 | Switched illustrative `Call` and `Return` definitions from single-variant enums to structs |
| 2026-02-11 | Added generic operation contracts and explicit high-level/lowered text-format roundtrip targets (including `bind`) |
| 2026-02-11 | Consolidated previous closure-lifting draft RFC semantics (hoisting, env parameter, specialization/backedge integration) into this RFC and renumbered this document to 0008 |
| 2026-02-11 | Audited callable operation semantics so v1 has no alias/duplicate callable-construction operation |
| 2026-02-11 | Renamed callable construction operation to `bind` for concise core syntax |
