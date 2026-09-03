# AGENTS.md

## Principles

- less standalone function is better
- every module only expects a few names to be imported, do not create giant sets of new names
- if we have a lot of implementations (over 200 lines), it is better to split them into multiple files.
- use `mod.rs` over `<name>.rs` for modules that contain multiple files.
- `mod.rs` should stay lean: only module declarations (`mod`), re-exports (`pub use`), and prelude definitions. Move substantial logic into sibling files within the same directory.
- when creating tests, always put common tools created for testing in the `kirin-test-utils` crate, unless they are specific to a single crate.
- **No unsafe code.** All implementations MUST use safe Rust. Do not use `unsafe` blocks, `mem::zeroed()`, `mem::transmute()`, `MaybeUninit`, raw pointers, or any other unsafe constructs. If a problem seems to require unsafe, redesign the approach to use safe alternatives (e.g., `Option` for tombstones, `enum` for tagged unions, bounds/trait constraints for type safety). Existing unsafe code is a bug to be fixed, not a pattern to follow.

## Build and Test

```bash
cargo build --workspace          # Build all crates
cargo nextest run --workspace    # Run all tests (preferred, parallelizes test binaries)
cargo nextest run -p kirin-chumsky  # Test a single crate
cargo nextest run -p kirin-derive-chumsky -E 'test(test_parse_add)'  # Run a single test
cargo test --doc --workspace     # Run doctests (nextest does not support doctests)
cargo fmt --all                  # Format code
cargo insta review               # Review snapshot test changes
cargo build -p toy-lang          # Build the toy language example binary
cargo run -p toy-lang -- parse example/toy-lang/programs/add.kirin  # Parse an example program from repo root
cargo run -p toy-lang -- run example/toy-lang/programs/add.kirin --stage source --function main 3 5  # Execute toy-lang main with i64 args
cargo run -p toy-lang -- run example/toy-lang/programs/branching.kirin --stage source --function abs --constprop 7  # Run constprop fixpoint analysis on toy-lang
cargo nextest run -p toy-lang    # Run toy language example tests
cargo build -p toy-qc            # Build the toy quantum-circuit example binary
cargo run -p toy-qc -- parse example/toy-qc/programs/bell_pair.kirin  # Parse a toy-qc example program from repo root
cargo nextest run -p toy-qc      # Run toy-qc example tests
cargo build -p kirin-interpreter  # Build the frame-fusion interpreter crate
cargo nextest run -p kirin-interpreter  # Run interpreter crate tests
cargo build -p kirin-derive-interpreter  # Build interpreter derive proc-macro crate
cargo nextest run -p kirin-derive-interpreter  # Run derive crate snapshot/unit tests
cargo nextest run -p toy-lang -E 'test(interpreter)'  # Run toy-lang interpreter tests
```

Rust edition 2024. No `rust-toolchain.toml`; uses the default toolchain.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`

Examples: `feat(chumsky): add cfg parser`, `fix(derive): handle empty enum variants`

Avoid large paragraphs in commit messages, keep them concise and focused on the changes made.

## Project structure

- `example` contains example code of the top-level crate `kirin`
- `tests` contains integration tests for the top-level crate `kirin`
- `crates` contains the crates that make up the project, most implementation can be found here.
- `docs/design` contains core design documents: syntax design, IR data structure design, text format specs, and semantic rule definitions. These are checked into git.
- `docs/plans` contains implementation plans. Checked into git.

### Subsystem Groupings

Named subsystem groupings for scoping implementation, review, and maintenance work:

| Subsystem | Crates |
|-----------|--------|
| `ir` | kirin-ir |
| `parser` | kirin-chumsky, kirin-derive-chumsky |
| `printer` | kirin-prettyless, kirin-derive-prettyless |
| `interpreter` | kirin-interpreter, kirin-derive-interpreter |
| `derive` | kirin-derive-toolkit, kirin-derive-ir, kirin-derive-chumsky, kirin-derive-interpreter, kirin-derive-prettyless |
| `dialects` | kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function |

### Dialect Domain Context

Each dialect crate targets a specific domain. Use this context when reviewing or changing dialect behavior:

| Crate(s) | Domain | Key References |
|-----------|--------|----------------|
| kirin-cf, kirin-scf | Compiler Engineering | Control flow graphs, SSA form, structured control flow (Cytron et al.), dominance, loop nesting |
| kirin-arith, kirin-bitwise, kirin-cmp | Numerics / Arithmetic | Type promotion rules, overflow semantics, IEEE 754, comparison semantics |
| kirin-function | PL / Lambda Calculus | Function application, closures, specialization, parametric polymorphism, calling conventions |
| kirin-constant | Compile-time Evaluation | Constant folding, staged computation, compile-time value semantics |
| kirin-ir (core) | Compiler IR Design | MLIR (Lattner et al. 2020), SSA form, regions/blocks/operations, arena-based IR |
| kirin-interpreter | Abstract Interpretation | Cousot & Cousot framework, lattice-based analysis, widening/narrowing, fixpoint computation; frame-fusion driver |

For user-defined dialects not in this table, ask the user for domain context during review planning.

### Crates

**Core:**
- `kirin-ir` — IR types, `Dialect` trait
- `kirin-lexer` — Logos tokenizer

**Parser/Printer:**
- `kirin-chumsky` — Parser traits (`HasParser`, `HasDialectParser`, `EmitIR`), text APIs (`ParseStatementText`, `ParsePipelineText`)
- `kirin-prettyless` — Pretty printer (`PrettyPrint`)
- `kirin-derive-chumsky` — `#[derive(HasParser, PrettyPrint)]` (proc-macro + code generation)

**Interpreter:**
- `kirin-interpreter` — interpreter framework. Shared pieces: `Interp`, `Interpretable<I, Semantics>`, `Frame`/`drive_frames`, and the owner-summary fixpoint driver (`StandardFixpointInterpreter`). **Semantics vs shape**: dialect rules dispatch on a `SemanticKey` (`ForwardEval`, `StrongDemand`, `ClassicLiveness`, or a downstream key — the Rust analogue of Kirin 1.0's string keys like `"main"`/`"typeinfer"`/`"constprop"`/`"qubit.address"`); each key declares the `AnalysisShape` its solver runs on (`SparseForwardShape`/`SparseBackwardShape`/`DenseForwardShape`/`DenseBackwardShape` — mechanics only, never dispatch tags). Two keys may share one shape. Each key joins its shape's *family* (`SparseForwardSemantic`/`SparseBackwardSemantic`/`DenseForwardSemantic`/`DenseBackwardSemantic`), and the engines/transfers are generic over the key with the canonical default (`SparseForwardTransfer<..., Sem = ForwardEval>`, `SparseBackwardInterpreter<..., Sem = StrongDemand>`, `DenseBackwardInterpreter<..., Sem = ClassicLiveness>`), so a downstream key reuses an engine by instantiating `Sem`. Engine traits are shape-generic mechanics (`SparseForwardInterp` read/write; `SparseBackwardInterp` fact/`raise_fact`/effect/topology; `DenseBackwardInterp` opaque `point_state`/`point_state_mut`); semantics-specific helper vocabulary lives in key-pinned helper traits (`DemandInterp`: `demand`/`is_demanded`/`demand_uses_if_observable`; `ClassicLivenessInterp`: `gen_live`/`kill_def`/`gen_uses_kill_defs`) — demand rules bind `DemandInterp`, classic-liveness rules bind `ClassicLivenessInterp`. **The shape layer never says what a fact is**: `raise_fact` takes the lattice element to merge, the dense point state is opaque, and the only state contracts the engines and dialect frames name are `Lattice` (merges) plus `DenseBackwardState` (`rename`/`forget`, for CFG edges and `scf.for`'s back-edge). Fact-shaped contracts are key-pinned instead — `HasTop` on `DemandInterp`, `PointFacts` on `ClassicLivenessInterp` — carried as associated-type bounds in the supertrait so elaboration keeps dialect rules from spelling them. Effects per shape: `SparseForwardEffect`, `SparseBackwardEffect`, `DenseBackwardEffect`; engines: `ConcreteInterpreter`, `SparseForwardInterpreter`, `SparseBackwardInterpreter`, `DenseBackwardInterpreter`. `InterpDispatch<I>` is keyed on the engine alone — the dispatched key is always `I::Semantics`, so a stage can never be paired with a foreign key's rules. `DenseForwardShape` (typestate) has no key yet. `AbstractInterpreter` is the marker trait for lattice-valued engines. **Source layout** (public API is unchanged — everything re-exports through `lib.rs` plus the `dialect`/`engine` preludes): `core/` (chassis: `Interp`/dispatch/effects/frame protocol/env/error/linker/queries), `semantics/` (`keys.rs` + `shape.rs`), `facts/` (`anchor.rs`/`store.rs`/`topology.rs`), `fixpoint/` (convergence driver), `engines/` (`concrete/`, `sparse_forward/`, `sparse_backward/`, `dense_backward/`, each `interp.rs` + optional `frames.rs`).

**Dialects:**
- `kirin-cf`, `kirin-scf`, `kirin-constant`, `kirin-arith`, `kirin-bitwise`, `kirin-cmp`, `kirin-function`

**Derive Infrastructure:**
- `kirin-derive-toolkit` — Shared derive utilities (IR model, darling re-export, template system)
- `kirin-derive-ir` — `#[derive(Dialect, StageMeta)]` and IR property traits
- `kirin-derive-interpreter` — `kirin-interpreter` derive proc macros (`#[derive(Interpretable)]`, `#[derive(FunctionEntry)]`, `#[derive(InterpDispatch)]`)
- `kirin-derive-prettyless` — `#[derive(RenderDispatch)]` (proc-macro)

**Analysis:**
- `kirin-interval` — Interval domain for abstract interpretation
- `kirin-liveness` — SSA liveness: strong demand (`analyze_demand`, sparse backward) + classic per-point (`analyze_dense`, dense backward), composable as `dense ∩ demanded`

**Testing:**
- `kirin-test-types` — Pure test type definitions (`UnitType`, `SimpleType`, `Value`)
- `kirin-test-languages` — Test language/dialect enums (`SimpleLanguage`, `ArithFunctionLanguage`, etc.)
- `kirin-test-utils` — Shared test helpers (`roundtrip`, `parser`, `lattice`, `rustfmt`)

## Derive Infrastructure Conventions

- **Darling re-export rule**: Derive crates that depend on `kirin-derive-toolkit` must use `kirin_derive_toolkit::prelude::darling` — never import `darling` directly. The workspace has multiple darling versions (0.20 via `bon`, 0.23 via `kirin-derive-toolkit`); a direct import may resolve to the wrong version.

- **Helper attribute pattern**: `#[wraps]` and `#[callable]` are intentionally separate from `#[kirin(...)]` for composability. `#[kirin(...)]` is the carry attribute for dialect-specific options (parsed by darling). `#[wraps]` is a generic helper for delegation/wrapper patterns, and `#[callable]` is interpreter-specific. Keeping them as bare attributes lets different derive macros compose independently — e.g. a type can use `#[wraps]` with both `#[derive(Dialect)]` and `#[derive(Interpretable)]` without coupling those derives. Since darling's `#[darling(attributes(...))]` only supports `#[attr(key = val)]` form, bare flag attributes are parsed manually via `attrs.iter().any(|a| a.path().is_ident("name"))`.

- **`#[wraps]` and `#[kirin(terminator)]` interaction**: When `#[wraps]` is per-variant, `is_terminator()` is automatically delegated to the inner type — no `#[kirin(terminator)]` needed. When `#[wraps]` is at enum level (all variants wrap), you still need explicit `#[kirin(terminator)]` on terminator variants. See `ArithFunctionLanguage` (per-variant, no terminator annotations) vs the inline `NumericLanguage` in `tests/roundtrip/arith.rs` (enum-level, explicit annotations).

- **Custom Layout for derive-specific attributes**: When a derive macro needs attributes beyond `StandardLayout` (which has `()` for all extras), define a custom `Layout` impl in that derive module. This keeps derive-specific attributes out of the core IR. See `EvalCallLayout` in `kirin-derive-interpreter` as an example.

- **Downstream crate path (`HasCratePath`)**: Each derive macro has its own crate path attribute — `#[kirin(crate = ...)]` is the IR crate, `#[chumsky(crate = ...)]` is the parser crate, `#[pretty(crate = ...)]` is the printer crate. These are independent. Implement `HasCratePath` on your `ExtraGlobalAttrs` and use `Input::extra_crate_path()` to resolve with a default.

- **Global-only fields in shared attribute namespaces**: When a derive's attribute namespace (e.g. `#[chumsky(...)]`) has fields that are global-only (like `crate`) but the same namespace is parsed at the statement/variant level, implement `Layout::extra_statement_attrs_from_input()` with a lenient intermediate struct (`#[darling(allow_unknown_fields)]`) that skips global-only fields. This keeps `FromVariant` strict — `#[chumsky(crate = ...)]` on a variant correctly errors — while tolerating it at the type level where both global and statement attrs share the namespace.

- **`#[kirin(...)]` attribute convention**: Use path syntax for `crate`: `#[kirin(crate = kirin_ir)]` not `#[kirin(crate = "kirin_ir")]`. Darling parses `syn::Path` and supports bare idents directly.

- **Auto-placeholder for `ResultValue` fields**: `ResultValue` fields without an explicit `#[kirin(type = ...)]` annotation automatically default to `ir_type::placeholder()`, where `ir_type` is the enum/struct-level `#[kirin(type = T)]` path. The derive adds `T: Placeholder` to generated builder and EmitIR where clauses automatically — dialect authors never write `+ Placeholder` on their struct definitions or interpret impls. Use explicit `#[kirin(type = expr)]` only when the result type is computed from other fields (e.g., `Constant`'s `#[kirin(type = value.type_of())]`).

## IR Design Conventions

- **Block vs CFG**: A `Block` is a single linear sequence of statements with an optional terminator. A `CFG` is a control-flow-graph body: a container for multiple blocks (`LinkedList<Block>`). (The `CFG` type was originally named `Region` after MLIR, but in Kirin it is specifically the block-list CFG body — single-block bodies use `Block`, and graph-like bodies use `DiGraph`/`UnGraph`.) When modeling MLIR-style operations, check whether the MLIR op uses `SingleBlock` regions — if so, use `Block` in Kirin, not `CFG`. For example, MLIR's `scf.if` and `scf.for` have `SingleBlock` + `SingleBlockImplicitTerminator<scf::YieldOp>` traits, so `kirin-scf` correctly uses `Block` fields for their bodies.

- **`BlockInfo::terminator` is a cached pointer**: The `terminator` field in `BlockInfo` is a cached pointer to the last statement in the block — it is NOT a separate statement. `StatementIter` only iterates the linked list of non-terminator statements. When querying the last statement, use `Block::last_statement(stage)` which returns `terminator.or_else(|| statements.tail())`. Do not assume the terminator is distinct from the statements list.

## Interpreter Conventions

- **Current framework**: Interpreter work belongs in `kirin-interpreter`. Dialect-specific implementations live in `src/interpreter.rs` inside each dialect crate. The design doc is `docs/design/interpreter/index.md`; update it when the framework changes.

- **`Interp` is the engine driver; `Interpretable<I, Semantics>` is the dialect trait**: `Interp` exposes `Value`, `Error`, `Effect`, `Semantics` (a `SemanticKey`), and the current statement location (`stage()`/`statement()`/`index()`). Dialect rules receive the engine `interp` directly and are selected by a compile-time semantic key (`ForwardEval`, `StrongDemand`, `ClassicLiveness`, downstream keys), not by a runtime context object and never by a raw solver shape. One dialect type carries one rule per key without coherence conflicts — including two keys on the same shape; a new analysis adds a new key (declaring its shape) + effect algebra instead of adding cases to an existing effect.

- **Two-persona contract**: Dialect authors implement `Interpretable<I, ForwardEval>` (and `FunctionEntry<I>` for callable statements). A rule receives the engine `interp: &mut I` directly and uses the `interp.read`/`interp.write` helpers, which are **default methods on `SparseForwardInterp`** operating on the engine's current activation (`interp.index()`); forward rules bound `I: SparseForwardInterp` so they can return `SparseForwardEffect` as `I::Effect`, plus value-domain bounds on `I::Value`. Compiler authors compose language enums with derives, pick a value type, error type, engine, and linker; when needed, they can also opt into custom frame types or custom abstract policies. Imports come from `kirin_interpreter::dialect` and `kirin_interpreter::engine` respectively. Customizing traversal is part of the compiler-author surface, not a separate persona.

- **Statement dispatch**: Dialect statements implement `Interpretable<I, ForwardEval>` — specialized on the engine `I` and the `ForwardEval` semantic key. A forward rule (`I: SparseForwardInterp`) reads/writes SSA state through the `SparseForwardInterp` **default-method** helpers (`interp.read`, `interp.write`, `interp.read_many`, `interp.write_results` — which delegate to the engine's `Env` storage access at `interp.index()`; there is no `ForwardCtx`/`ValueContext` type) and returns `Result<I::Effect, I::Error>`, building `SparseForwardEffect`: atomic ops return `SparseForwardEffect::Next`; control ops return `Jump`/`Branch` (CFG edges), `Call`, `Yield`/`Return` (completions), or `Push` (run a sub-computation by pushing a dialect-owned frame, then bind its results). There is **no** framework "scope" type and no framework "explore alternatives" effect.

- **Dialects are engine-blind**: one `Interpretable` impl serves concrete execution and abstract interpretation; the value domain decides. Undecided conditions (`BranchCondition::is_truthy` / `ForLoopValue::loop_condition` returning `None`) are read in the rule and handed to the dialect's own frame, which rejects them under concrete execution and explores+joins under abstract. (`Branch` is the cf CFG analogue, driven by the engine's CFG frame.) Never write per-engine dialect impls — but a control dialect's *frame* may have distinct concrete/abstract forms, built per-engine through a dialect dispatch trait.

- **Ordinary vs control dialects (frame ownership)**: Ordinary dialects (arith, cmp, constant, bitwise, tuple, ordinary cf branch ops) implement statement-local semantics with the `SparseForwardInterp` helpers and **never see frames**. A dialect whose operations own *structured traversal* defines **dialect-owned frames** and pushes them with `SparseForwardEffect::Push`. The framework's `BlockFrame` / `AbstractBlockFrame` (single-block body walkers) are reusable **building blocks**, not framework-owned structured semantics — a dialect frame may build one to walk a chosen body, but the structured *decision* and result binding stay in the dialect frame.

- **SCF is the example**: `scf.if` → `kirin_scf::ScfIfFrame` (concrete) / `AbstractScfIfFrame` (abstract); `scf.for` → `ScfForFrame` / `AbstractScfForFrame`. Each is selected per engine through a dialect dispatch trait (`ScfIfDispatch`/`ScfForDispatch`) and returned as `SparseForwardEffect::Push`. The if frame owns picking the arm (concrete) or exploring both arms + joining (abstract); the for frame owns the loop-carried join/widen fixpoint. A language that uses SCF includes the corresponding SCF continuations in its private stack-item composition and owns the narrow `From<Scf*Frame>` conversions. SCF frames return themselves for local continuation and require only conversions for the child walkers they push; neither the dialect nor a member continuation constructs an enclosing enum variant. See `example/toy-lang`'s private concrete and dense stack-item enums and its current `ToyAbstractFrame` composition. (Future structured dialects follow the same ownership rule; only the existing SCF operations are implemented.)

- **Calling conventions are linkers**: `Linker<S>` resolves `Callee` to a `(stage, specialization, body)` target and is passed to engines by value (`.with_linker(..)`). `SameStageLinker` is the default; `CrossStageLinker` routes calls to whichever stage has a live specialization, which is all that cross-language execution *and* cross-language analysis require. Policy must be a component (field), never a trait impl on an engine type.

- **Engines run frames; traversal lives in frames**: all frame-using engines share one driver loop, `drive_frames` (`core/frame.rs`), over the direction-neutral `Frame<I: FrameEngine, F = Self>` protocol — pop the top stack item, call `step_into`, and apply its `FrameEffect`, while owning no representation traversal itself. A reusable *member continuation* returns `Self` for `Continue` and `Push.parent`; the generic currently named `F` is only the configured representation of a differently typed pushed child. A private closed *frame stack item* enum is the composition root: it owns `From<Member>` conversions, exhaustively dispatches to members, and uses `FrameEffect::map_next` to wrap returned member state. `drive_frames` sees only the homogeneous stack-item type and requires it to use itself as its child representation. `FrameEngine` remains the minimal anchor (just a total `Error`), so this protocol is independent of forward evaluation. `ConcreteInterpreter` hides the framework-default concrete stack item; a language with additional continuations wraps `ConcreteInterpreterCore<.., FrameStackItem>` behind its own API. Sparse-forward currently exposes `StandardAbstractFrame` as its default composition pending the separate facade/core review. Dense backward uses `DenseBlockFrame` directly by default and a language-private stack-item enum when structured continuations coexist. Sparse backward's `DemandFrame` is already its complete homogeneous composition because it never pushes a differently typed child. Analysis crates remain a lattice plus an engine/policy/composition choice (see `kirin-constprop` and `kirin-liveness`).

- **Call-body traversal is concrete configuration**: `CallFrame` owns the call convention (resolve, allocate the activation, enter, suspend, validate the completion, free exactly once, bind results) and delegates *only* which child continuation enters the callee body to `CallBodyTraversal<V, E, F>`. The private composition root selects the traversal as `T` in its `CallFrame<V, T>` stack-item variant; `CallFrame<V>` means `CallFrame<V, DefaultCallBodyTraversal>`. `CallRequest` carries no `T`; converting it into the configured stack-item attaches that compile-time choice. **Concrete execution only** — forward abstract interpretation summarizes calls (`AbstractCallFrame`) and maps a callable body to an `Owner` instead of descending, and the backward engines never walk callable bodies through a call frame. This traversal is not consulted for nested bodies: `scf.if`/`scf.for` and other structured operations keep choosing their own dialect continuations.

- **Engine capabilities are per-frame, not per-engine**: `core/frame.rs` splits the forward engine surface into component traits named after the *capability* they supply — `StatementDispatch: Interp` (dispatch a statement), `BlockQueries: Interp` (read-only block queries), `CFGQueries: BlockQueries` (`cfg_entry`), `DiGraphQueries: Interp` (`digraph_walk_plan`), `CallServices: Env` (activation storage, linking, callable-entry dispatch; kept whole because `CallFrame` consumes all four and their pairing is a safety property — `CallFrame` still owns the *convention*). **A member frame bounds only what it consumes** (`ScfIfFrame` needs just `FrameEngine<Error = E>`; `ScfForFrame` just `Env`). A stack-item composition's `Frame` implementation requires the union of its variants' member bounds, because one engine must be able to run every continuation admitted to that stack. `ForwardDataflowFrameEngine` extends only `Env + StatementDispatch + BlockQueries + DiGraphQueries` — an abstract engine summarizes calls and seeds owners, so it must **not** be made to inherit `CallServices` or `CFGQueries`. `tests/frame_engine_capabilities.rs` holds mock engines whose ability to compile is the regression test; widening a member frame's bound breaks it.

- **Naming rule for these traits**: `drive_frames` is the only *driver* at this layer (the frame-stack loop); `ForwardDriver`/`DenseBackwardDriver` are fixpoint-driver structs. Capability traits are named for what they supply, never `*FrameDriver` — do not reintroduce that suffix, and do not add compatibility aliases for it. `FrameEngine` = minimal contract for the generic frame stack; `ForwardFrameEngine`/`ForwardDataflowFrameEngine`/`DenseBackwardFrameEngine` = capability sets required by their corresponding families of computations. The `*Queries` traits must stay **read-only** and require only `Interp`: block-entry binding lives on the crate-private `BlockBinding: Env + BlockQueries` so no query trait's name hides a store mutation. Binding into an explicitly named activation is `Env::bind_values(index, slots, values)`; `SparseForwardInterp::write_results` is the dialect-facing current-activation helper. Names describe the operation, not the `Product` container.

- **`StatementDispatch` vs `InterpDispatch`**: opposite directions. `InterpDispatch<I>` is implemented by a *stage/language* to route a statement to its dialect rule. `StatementDispatch` is implemented by the *engine* and is what a frame calls: it stashes the current location (`stage`/`statement`/`index`) for the rule to read back through `Interp`, then delegates to `InterpDispatch`.

- **Customizing traversal**: Every member continuation and stack-item composition implements the same three methods (`step_into`/`resume_done_into`/`resume_into`). A reusable member returns `FrameEffect<Self, Completion, F>`, where `F` is its configured child representation; a stack-item enum dispatches to the member and maps `Self` into the matching variant. Concrete and language-specific compositions list their admitted framework/dialect continuations explicitly, provide narrow conversions from the member continuations or entry requests they accept, and keep the stack-item enum private where the public engine API permits. Callable-body walker selection remains the separate `CallBodyTraversal` configuration seam; structured dialects may push dialect-owned frames through `SparseForwardEffect::Push`; ordinary dialects never name frame types. Abstract summary keying and join/widen policy stay in `CallContext`/`WideningStrategy`.

- **Stage dispatch**: stage enums add `#[derive(InterpDispatch)]` next to `StageMeta`/`ParseDispatch`; single-language pipelines get a blanket impl. `InterpDispatch<I>` is keyed on the engine alone; the dispatched key is always `I::Semantics`. The engine sets its current location then passes itself to dispatch, which forwards to the matching `Interpretable`/`FunctionEntry` rule. Engine-internal IR queries go through `StageQuery`.

- **Products and multi-result**: `kirin_ir::Product<T>` is the framework packet for call/block/branch arguments, function returns, and SCF yields. `HasProductValue` is only for value domains that expose an explicit tuple runtime value (the tuple dialect); it is not needed for ordinary multi-result plumbing.

- **Derive naming rule**: every interpreter derive is named after the trait it implements (`Interpretable`, `FunctionEntry`, `InterpDispatch`). Do not add derives whose names are not trait names.

- **Function dialect naming**: `kirin_function::Function<T>` is the standard function statement. New code should use `Function<T>` with `FunctionEntry` and `SparseForwardEffect::Call`/`SparseForwardEffect::Return`.

- **Backward analyses (implemented)**: liveness ships as **two** analyses in `kirin-liveness`, both real framework clients. *Strong liveness* (`analyze_demand`, `StrongDemand`) is per-SSA-value demand: summary owners ARE scope-qualified SSA values (`Scoped<(CompileStage, CFG), SSAValue>`), the driver's default self-dependent index is the demand worklist, ordinary dialects are a one-liner (`interp.demand_uses_if_observable(self)` on `DemandInterp` — purity-aware via `IsPure`: impure statements and terminator/return operands are roots), and SCF needs no heterogeneous continuation stack (loop-carried demand converges on the value worklist; the SCF rules use `block_params`/`terminator_args` queries). *Classic per-point liveness* (`analyze_dense`, `ClassicLiveness`) is the textbook kill-defs/gen-all-uses transfer over block owners with backward block walks (`DenseBlockFrame`, `absorb_edges` maps successor live-ins across edges with pass-through for dominated direct cross-block uses); SCF owns dense member continuations (`DenseScfIfFrame` arm join, `DenseScfForFrame` loop fixpoint), and a language-private stack-item enum composes them with `DenseBlockFrame` through narrow `From<Member>` conversions (see toy-lang's `ToyDenseBackwardFrame`). Strong per-point sets are the composition `dense ∩ demanded`, not a third analysis. CFG topology (blocks incl. nested bodies, feeders) comes from `StageQuery` actions — enumeration only; use/def/edge-arg *semantics* stay in dialect rules. One dialect carries one rule per semantic key without coherence conflicts, and two keys can share one shape — the shipped dialects (each carrying `ForwardEval` + `StrongDemand` + `ClassicLiveness` rules) are the living evidence. Dense point-observation ownership and the adequacy of block owners/body coverage are explicitly deferred architecture questions, not claims established by the current composition.

## Chumsky Parser Conventions

- **Single lifetime `HasParser<'t>`**: All parser traits use a single lifetime `'t` (the input text lifetime). The old two-lifetime system (`HasParser<'tokens, 'src>`) has been collapsed. `HasDialectParser<'t>` has 4 required items: `Output` type, `namespaced_parser`, `clone_output`, `eq_output` — `recursive_parser` has a default impl.

- **`ParseEmit<L>` for text parsing APIs**: `ParseStatementText` and `ParsePipelineText` require `L: ParseEmit<L>`. Three implementation paths: (1) `#[derive(HasParser)]` generates it automatically, (2) implement `SimpleParseEmit` marker for non-recursive dialects to get a blanket impl, (3) implement `ParseEmit` directly for full control. The derive-generated impl delegates to internal `HasParserEmitIR<'t>` (not in the public API) because GAT projection normalization requires a concrete lifetime parameter.

- **`ParseDispatch` for pipeline parsing**: Multi-dialect pipeline parsing uses `ParseDispatch` (a monomorphic dispatch trait) instead of HRTB-based `SupportsStageDispatchMut`. Add `#[derive(ParseDispatch)]` alongside `#[derive(StageMeta)]` on stage enums. Single-dialect pipelines (`Pipeline<StageInfo<L>>`) get a blanket `ParseDispatch` impl. Zero HRTB in the dispatch chain.

- **`#[wraps]` works with CFG/Block-containing types**: Dialect types that contain `CFG` or `Block` fields (e.g., `Lambda`, `Function`, SCF operations) can be composed via `#[wraps]` + `HasParser`. See `example/toy-lang/src/language.rs` where `Lexical` (contains `Function` with a CFG body and `Lambda` with a CFG body) and `StructuredControlFlow` (contains `If`/`For` with Block fields) are both used with `#[wraps]`.

- **`Ctx` default parameter for unified traits**: When the same trait method needs extra context for some implementors (e.g., `CompileStage` for `Pipeline`) but not others (e.g., `StageInfo`), use a default type parameter `Ctx = ()` on the trait. Pair with a blanket `Ext` trait that erases the `()` arg for ergonomic call sites. See `ParseStatementText<L, Ctx>` / `ParseStatementTextExt<L>`.

## Test Conventions

- **Roundtrip tests** (parse → emit → print → compare) go in workspace `tests/roundtrip/<dialect>.rs`
- **Unit tests** for internal logic go inline in the crate (`#[cfg(test)]`)
- **Codegen snapshot tests** go inline in `kirin-derive-chumsky`
- **IR rendering snapshots** go inline in `kirin-prettyless`
- **New test types** (type lattices, values) go in `kirin-test-types`
- **New test dialects** (language enums, stage enums) go in `kirin-test-languages`
- **New test helpers** (roundtrip, parse, fixture builders) go in `kirin-test-utils`
