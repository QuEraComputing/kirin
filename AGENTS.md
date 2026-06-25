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

Examples: `feat(chumsky): add region parser`, `fix(derive): handle empty enum variants`

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
- `kirin-interpreter` — interpreter framework. Shared pieces: `Interp`, `InterpretCtx`, `Interpretable<C>`, `Frame`, and `drive_frames`. Forward pieces: `ForwardContext`, `Env`, `ForwardEffect`, `ConcreteInterpreter`, and `ForwardAbstractInterpreter`. `AbstractInterpreter` is the marker trait for lattice-valued engines.

**Dialects:**
- `kirin-cf`, `kirin-scf`, `kirin-constant`, `kirin-arith`, `kirin-bitwise`, `kirin-cmp`, `kirin-function`

**Derive Infrastructure:**
- `kirin-derive-toolkit` — Shared derive utilities (IR model, darling re-export, template system)
- `kirin-derive-ir` — `#[derive(Dialect, StageMeta)]` and IR property traits
- `kirin-derive-interpreter` — `kirin-interpreter` derive proc macros (`#[derive(Interpretable)]`, `#[derive(FunctionEntry)]`, `#[derive(InterpDispatch)]`)
- `kirin-derive-prettyless` — `#[derive(RenderDispatch)]` (proc-macro)

**Analysis:**
- `kirin-interval` — Interval domain for abstract interpretation

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

- **Block vs Region**: A `Block` is a single linear sequence of statements with an optional terminator. A `Region` is a container for multiple blocks (`LinkedList<Block>`). When modeling MLIR-style operations, check whether the MLIR op uses `SingleBlock` regions — if so, use `Block` in Kirin, not `Region`. For example, MLIR's `scf.if` and `scf.for` have `SingleBlock` + `SingleBlockImplicitTerminator<scf::YieldOp>` traits, so `kirin-scf` correctly uses `Block` fields for their bodies.

- **`BlockInfo::terminator` is a cached pointer**: The `terminator` field in `BlockInfo` is a cached pointer to the last statement in the block — it is NOT a separate statement. `StatementIter` only iterates the linked list of non-terminator statements. When querying the last statement, use `Block::last_statement(stage)` which returns `terminator.or_else(|| statements.tail())`. Do not assume the terminator is distinct from the statements list.

## Interpreter Conventions

- **Current framework**: Interpreter work belongs in `kirin-interpreter`. Dialect-specific implementations live in `src/interpreter.rs` inside each dialect crate. The design doc is `docs/design/interpreter/index.md`; update it when the framework changes.

- **`Interp` is the engine driver; `Interpretable<C>` is the dialect trait**: `Interp` exposes `Value`, `Error`, `Effect`, and `Context<'a>`. Dialect rules specialize on a context type, not directly on an engine type. Forward rules use `Interpretable<ForwardContext<'_, I>>`; future analyses should define their own context type instead of adding more cases to `ForwardEffect`.

- **Two-persona contract**: Dialect authors implement `Interpretable<ForwardContext<'_, I>>` (and `FunctionEntry<ForwardContext<'_, I>>` for callable statements) using the **inherent** `ctx.read`/`ctx.write` helpers on `ForwardContext` (no helper-trait import — the helpers are inherent), value-domain bounds on `I::Value`, and — for forward rules — `I: ForwardInterp` so they can return `ForwardEffect` as `I::Effect`. Compiler authors compose language enums with derives, pick a value type, error type, engine, and linker; when needed, they can also opt into custom frame types or custom abstract policies. Imports come from `kirin_interpreter::dialect` and `kirin_interpreter::engine` respectively. Customizing traversal is part of the compiler-author surface, not a separate persona.

- **Statement dispatch**: Dialect statements implement `Interpretable<ForwardContext<'_, I>>` — specialized on the forward context type; `C::Value`/`C::Error`/`C::Effect` resolve (for `C = ForwardContext<'_, I>`) to `I::Value`/`I::Error`/`I::Effect`. A forward rule (`I: ForwardInterp`) reads/writes SSA state through `ForwardContext`'s **inherent** helpers (`ctx.read`, `ctx.write`, `ctx.read_many`, `ctx.write_results` — which delegate to the engine's `Env` storage access; there is no `ForwardCtx` trait) and returns `Result<I::Effect, I::Error>`, building `ForwardEffect`: atomic ops return `ForwardEffect::Next`; control ops return `Jump`/`Branch` (CFG edges), `Call`, `Yield`/`Return` (completions), or `Push` (run a sub-computation by pushing a dialect-owned frame, then bind its results). There is **no** framework "scope" type and no framework "explore alternatives" effect.

- **Dialects are engine-blind**: one `Interpretable` impl serves concrete execution and abstract interpretation; the value domain decides. Undecided conditions (`BranchCondition::is_truthy` / `ForLoopValue::loop_condition` returning `None`) are read in the rule and handed to the dialect's own frame, which rejects them under concrete execution and explores+joins under abstract. (`Branch` is the cf CFG analogue, driven by the engine's CFG frame.) Never write per-engine dialect impls — but a control dialect's *frame* may have distinct concrete/abstract forms, built per-engine through a dialect dispatch trait.

- **Ordinary vs control dialects (frame ownership)**: Ordinary dialects (arith, cmp, constant, bitwise, tuple, ordinary cf branch ops) implement statement-local semantics with `ForwardContext` and **never see frames**. A dialect whose operations own *structured traversal* defines **dialect-owned frames** and pushes them with `ForwardEffect::Push`. The framework's `BodyFrame` / `AbstractBlockFrame` (single-block body walkers) are reusable **building blocks**, not framework-owned structured semantics — a dialect frame may build one to walk a chosen body, but the structured *decision* and result binding stay in the dialect frame.

- **SCF is the example**: `scf.if` → `kirin_scf::ScfIfFrame` (concrete) / `AbstractScfIfFrame` (abstract); `scf.for` → `ScfForFrame` / `AbstractScfForFrame`. Each is built per-engine through a dialect dispatch trait (`ScfIfDispatch`/`ScfForDispatch`) and returned as `ForwardEffect::Push`. The if frame owns picking the arm (concrete) or exploring both arms + joining (abstract); the for frame owns the loop-carried join/widen fixpoint. A language that uses SCF composes a total frame type embedding the standard frames plus `ScfIfFrame`/`ScfForFrame` (via `BuildScfIf`/`BuildScfFor` and the abstract equivalents); see `example/toy-lang`'s `ToyFrame`/`ToyAbstractFrame`. (Future structured dialects would follow the same pattern; only the existing SCF ops are implemented.)

- **Calling conventions are linkers**: `Linker<S>` resolves `Callee` to a `(stage, specialization, body)` target and is passed to engines by value (`.with_linker(..)`). `SameStageLinker` is the default; `CrossStageLinker` routes calls to whichever stage has a live specialization, which is all that cross-language execution *and* cross-language analysis require. Policy must be a component (field), never a trait impl on an engine type.

- **Engines run frames; traversal lives in frames**: both engines share one driver loop, `drive_frames` (`frame.rs`), over the direction-neutral `Frame<I: FrameEngine>` protocol — pop the top frame, `step`, apply the returned `FrameEffect`, owning no traversal logic. `FrameEngine` is the minimal anchor (just a total `Error`); every `Interp` is a `FrameEngine` by blanket impl, so frames are decoupled from the forward value engine and stay reusable. `ConcreteInterpreter<'ir, S, V, E, Lk, F = StandardFrame<V, E>>` uses the concrete standard frames (`concrete_frame.rs`: `BodyFrame`/`CallFrame`, single-path). `ForwardAbstractInterpreter<'ir, S, V, E, Lk, P = ContextInsensitive, F = StandardAbstractFrame<V, E, P::Key>>` (the forward abstract engine, implementing the `AbstractInterpreter` trait) uses the abstract standard frames (`abstract_frame.rs`: `AbstractFunctionFrame`/`AbstractCfgFrame`/`AbstractBlockFrame`/`AbstractCallFrame`) — block-worklist CFG with widening, `Branch` exploration, and per-key interprocedural summaries (caller re-enqueueing incl. same-key self-recursion). A future `BackwardAbstractInterpreter` for liveness would implement the same `AbstractInterpreter` trait with its own context/fact store/effect/frames. The default `StandardFrame`/`StandardAbstractFrame` are structured-control-free; a language with a structured dialect supplies a custom `F` embedding the standard frames (via `FrameBuild`/`AbstractFrameBuild`) plus that dialect's frames. Analysis crates are a lattice + an analysis policy + a `ForwardAbstractInterpreter` type alias (see `kirin-constprop`, a forward abstract-interpretation specialization).

- **Customizing traversal**: `frame.rs` defines the shared `Frame`/`FrameEffect`/`drive_frames` protocol plus forward driver traits. Concrete custom frames embed `BodyFrame`/`CallFrame` via `FrameBuild`; forward abstract custom frames embed `Abstract*Frame`s via `AbstractFrameBuild`. Structured dialects may push dialect-owned frames with `ForwardEffect::Push`; ordinary dialects never name frame types. Abstract summary keying and join/widen policy stay in `CallContext`/`WideningStrategy`.

- **Stage dispatch**: stage enums add `#[derive(InterpDispatch)]` next to `StageMeta`/`ParseDispatch`; single-language pipelines get a blanket impl. `InterpDispatch<C>` is keyed on the context type `C: InterpretCtx`. The engine builds a context and dispatch forwards it to the matching `Interpretable`/`FunctionEntry` rule. Engine-internal IR queries go through `StageQuery`.

- **Products and multi-result**: `kirin_ir::Product<T>` is the framework packet for call/block/branch arguments, function returns, and SCF yields. `HasProductValue` is only for value domains that expose an explicit tuple runtime value (the tuple dialect); it is not needed for ordinary multi-result plumbing.

- **Derive naming rule**: every interpreter derive is named after the trait it implements (`Interpretable`, `FunctionEntry`, `InterpDispatch`). Do not add derives whose names are not trait names.

- **Function dialect naming**: `kirin_function::Function<T>` is the standard function statement. New code should use `Function<T>` with `FunctionEntry` and `ForwardEffect::Call`/`ForwardEffect::Return`.

- **Future analyses define their own context type + effect algebra**: for example, liveness should use a `LivenessContext<'_, I>`, liveness-specific helpers, its own `Effect`, and its own frame-driver capability. It should not add backward variants to `ForwardEffect`. A compile-time readiness proof for this shape lives in `kirin-interpreter::dispatch::tests`.

## Chumsky Parser Conventions

- **Single lifetime `HasParser<'t>`**: All parser traits use a single lifetime `'t` (the input text lifetime). The old two-lifetime system (`HasParser<'tokens, 'src>`) has been collapsed. `HasDialectParser<'t>` has 4 required items: `Output` type, `namespaced_parser`, `clone_output`, `eq_output` — `recursive_parser` has a default impl.

- **`ParseEmit<L>` for text parsing APIs**: `ParseStatementText` and `ParsePipelineText` require `L: ParseEmit<L>`. Three implementation paths: (1) `#[derive(HasParser)]` generates it automatically, (2) implement `SimpleParseEmit` marker for non-recursive dialects to get a blanket impl, (3) implement `ParseEmit` directly for full control. The derive-generated impl delegates to internal `HasParserEmitIR<'t>` (not in the public API) because GAT projection normalization requires a concrete lifetime parameter.

- **`ParseDispatch` for pipeline parsing**: Multi-dialect pipeline parsing uses `ParseDispatch` (a monomorphic dispatch trait) instead of HRTB-based `SupportsStageDispatchMut`. Add `#[derive(ParseDispatch)]` alongside `#[derive(StageMeta)]` on stage enums. Single-dialect pipelines (`Pipeline<StageInfo<L>>`) get a blanket `ParseDispatch` impl. Zero HRTB in the dispatch chain.

- **`#[wraps]` works with Region/Block-containing types**: Dialect types that contain `Region` or `Block` fields (e.g., `Lambda`, `Function`, SCF operations) can be composed via `#[wraps]` + `HasParser`. See `example/toy-lang/src/language.rs` where `Lexical` (contains `Function` with Region and `Lambda` with Region) and `StructuredControlFlow` (contains `If`/`For` with Block fields) are both used with `#[wraps]`.

- **`Ctx` default parameter for unified traits**: When the same trait method needs extra context for some implementors (e.g., `CompileStage` for `Pipeline`) but not others (e.g., `StageInfo`), use a default type parameter `Ctx = ()` on the trait. Pair with a blanket `Ext` trait that erases the `()` arg for ergonomic call sites. See `ParseStatementText<L, Ctx>` / `ParseStatementTextExt<L>`.

## Test Conventions

- **Roundtrip tests** (parse → emit → print → compare) go in workspace `tests/roundtrip/<dialect>.rs`
- **Unit tests** for internal logic go inline in the crate (`#[cfg(test)]`)
- **Codegen snapshot tests** go inline in `kirin-derive-chumsky`
- **IR rendering snapshots** go inline in `kirin-prettyless`
- **New test types** (type lattices, values) go in `kirin-test-types`
- **New test dialects** (language enums, stage enums) go in `kirin-test-languages`
- **New test helpers** (roundtrip, parse, fixture builders) go in `kirin-test-utils`
