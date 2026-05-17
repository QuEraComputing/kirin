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
cargo run -p toy-lang -- run example/toy-lang/programs/add.kirin --stage source --function main --new-interpreter 3 5  # Execute via interpreter-new concrete path
cargo run -p toy-lang -- run example/toy-lang/programs/branching.kirin --stage source --function abs --new-constprop 7  # Run interpreter-new constprop fixpoint analysis
cargo nextest run -p toy-lang    # Run toy language example tests
cargo build -p toy-qc            # Build the toy quantum-circuit example binary
cargo run -p toy-qc -- parse example/toy-qc/programs/bell_pair.kirin  # Parse a toy-qc example program from repo root
cargo nextest run -p toy-qc      # Run toy-qc example tests
cargo build -p kirin-interpreter-new  # Build the new frame-fusion interpreter crate
cargo nextest run -p kirin-interpreter-new  # Run new interpreter crate tests
cargo nextest run -p toy-lang -E 'test(interpreter_new)'  # Run toy-lang new interpreter tests
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
| `interpreter` | kirin-interpreter, kirin-interpreter-new, kirin-derive-interpreter |
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
| kirin-interpreter-new | Abstract Interpretation | Cousot & Cousot framework, lattice-based analysis, widening/narrowing, fixpoint computation |

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
- `kirin-interpreter` — legacy interpreter traits, `StackInterpreter`, `AbstractInterpreter`
- `kirin-interpreter-new` — current frame-fusion interpreter framework for concrete and abstract interpretation
- `kirin-derive-interpreter` — legacy `#[derive(Interpretable, CallSemantics)]`

**Dialects:**
- `kirin-cf`, `kirin-scf`, `kirin-constant`, `kirin-arith`, `kirin-bitwise`, `kirin-cmp`, `kirin-function`

**Derive Infrastructure:**
- `kirin-derive-toolkit` — Shared derive utilities (IR model, darling re-export, template system)
- `kirin-derive-ir` — `#[derive(Dialect, StageMeta)]` and IR property traits
- `kirin-derive-prettyless` — `#[derive(RenderDispatch)]` (proc-macro)

**Analysis:**
- `kirin-interval` — Interval domain for abstract interpretation

**Testing:**
- `kirin-test-types` — Pure test type definitions (`UnitType`, `SimpleType`, `Value`)
- `kirin-test-languages` — Test language/dialect enums (`SimpleLanguage`, `CompositeLanguage`, `ArithFunctionLanguage`, etc.)
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

- **Current framework**: New interpreter work belongs in `kirin-interpreter-new`. Dialect-specific implementations live in `src/interpreter_new.rs` or `src/interpreter_new/mod.rs` inside each dialect crate. The old `kirin-interpreter-20` crate and downstream `interpreter20` modules have been removed; do not add new code against them.

- **Design source**: The checked-in design for the current framework is under `docs/design/new-interpreter/`. Do not revive stale interpreter-2/interpreter-4/interpreter-9 design documents; update the new-interpreter docs instead.

- **Frame fusion model**: A `Frame` is a continuation object anchored at a semantic-independent `Location`. `ConcreteInterpreter` and abstract drivers own the driver loop; frames consume `self` in `step`, `resume_done`, and `resume`, then return `FrameEffect<F, C>`. This keeps the interpreter stack explicit and avoids using the Rust call stack for interpreter control flow.

- **Statement dispatch**: Dialect statements implement `Interpretable<L, I, F, C, E, T>` with `Interpretable::interpret`. Atomic statements mutate SSA state through `Env` and return `StatementEffect::Done`. Non-atomic statements return `StatementEffect::Push(frame)`, `StatementEffect::Transfer(transfer)`, or `StatementEffect::Complete(completion)`.

- **Env access**: `Env<V>` is the SSA store capability. It exposes `alloc`, `free`, `read`, `write`, `read_many`, and `write_product` using explicit `EnvIndex` values. `read_many` returns `Product<V>`, not `Vec<V>`. Concrete execution typically uses `EnvStackStore<V>`; abstract execution can use `AbstractEnvStore<V>` or another store through `AbstractInterpreterWithStore`.

- **Products and multi-result**: `kirin_ir::Product<T>` is the framework packet for call arguments, block arguments, branch arguments, function returns, and SCF yields. `StandardCompletion::FunctionReturned(Product<V>)` and `ScfCompletion::Yield(Product<V>)` carry products directly. Use `Env::write_product` to destructure into SSA result slots. `HasProductValue` is only for value domains that expose an explicit tuple/product runtime value for the tuple dialect; it is not required for ordinary multi-result return/yield plumbing.

- **Concrete vs abstract transfers**: Transfer payloads are frame/interpreter-specific. Concrete CFG execution uses `ConcreteBlockTransfer<V>` with `Jump`. Abstract execution can use `AbstractBlockTransfer<V>` with `Jump` and `Branch`; unknown branches become `Branch` and are handled by abstract branch frames or by a fixpoint owner strategy.

- **Standard frames**: Reuse standard frames from `kirin-interpreter-new::standard` for common IR traversal and call conventions: `StatementFrame`, `BlockFrame`, `RegionFrame`, `CallFrame`, `FunctionFrame`, `StagedFunctionFrame`, and `SpecializedFunctionFrame`. `RegionFrame` follows CFG convention: it enters the entry block and subsequent block movement is driven by block transfers, not by iterating region blocks.

- **Function dialect naming**: `kirin_function::Function<T>` is the standard function statement. `FunctionBody<T>` exists only as a deprecated compatibility alias. New code should use `Function<T>`, `FunctionEntry`, and the standard function/call frames.

- **Lift/project algebra**: Frame, completion, error, and summary composition should use lift/project traits, not `From`/`Into`, when the direction matters. Use `.lift()` only for infallible lifts, like `.into()`, and `.try_lift()` for fallible lifts, like `.try_into()`. Infallible `TryLiftFrom` impls should use `Error = core::convert::Infallible`. Error composition should usually be `E: LiftFrom<SomeError>` and call `E::lift_from(error)`.

- **Abstract interpretation**: `SimpleFixpointInterpreter` is owner-summary based: an owner maps to a `Summary`, `OwnerSemantics` builds the entry frame and merges completions back into summaries, and `Summary::merge` owns join/widen/narrow behavior. Use this for fixpoint analyses; use direct `AbstractInterpreter` only for single abstract executions that do not need owner-summary convergence.

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
