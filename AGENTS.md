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
cargo nextest run -p toy-lang    # Run toy language example tests
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
- `docs/review` contains review reports from triage-review and test-coverage-review skills, organized as `<date>/` with per-crate subdirectories. Checked into git.
- `docs/plans` contains implementation plans. Checked into git.
- `docs/superpowers/` contains brainstorming artifacts and other skill-generated working files. This directory is gitignored — do not check these files in.
- `.agents` contains agent specific implementations that is not included in this file, e.g skills.
- `.agents/team` contains reviewer and implementer persona definitions used by review and refactoring skills.
- `.agents/team/templates` contains saved team configurations from past refactors.
- `skills/` contains project-specific skill definitions. Managed via `ion-cli` (see Skill Management below).

### Skill Management

Skills are managed using `ion-cli`. Use `ion-cli --json` for structured, non-interactive control of skill installation, search, and project management. Load the `ion-cli` skill for details on available commands.

### Skill Architecture

Skills are organized into composable layers. Higher layers compose lower layers — never the reverse.

```
Layer 4: Orchestrators (compose lower layers into end-to-end workflows)
  feature-dev — design + implement new capabilities, scales from small to framework-level
  dialect-dev — kirin-specific: text format + semantics → IR → parser → printer → interpreter
  refactor — restructure existing code with architectural guardrails

Layer 3: Review & Discovery (read-only, produce reports)
  triage-review — multi-perspective codebase review with cross-review validation
  test-coverage-review — discovers issues by writing tests, reports findings

Layer 2: Workflow Steps (reusable building blocks)
  brainstorming — open design exploration
  writing-plans — structured implementation plan creation
  subagent-driven-development — same-session task execution with per-task review
  executing-plans — cross-session/parallel plan execution
  finishing-a-development-branch — completion, CI, merge

Layer 1: Primitives (atomic utilities)
  dispatching-parallel-agents — parallel agent orchestration with isolation
  ask-user-question — structured user interviews
  insta-snapshot-testing — snapshot test workflow
  using-git-worktrees — worktree lifecycle management
  verification-before-completion — pre-completion verification checks
  skill-health — audit local skills for drift, convention violations, missing benchmarks

Domain-Specific (standalone, composable with any layer):
  kirin-derive-macros — derive macro development guidance
  ir-spec-writing — IR specification design
```

**Composition rules:**
- Layer 3 skills are **read-only** — they produce reports and findings but do not modify code.
- Layer 4 orchestrators compose Layer 3 (for review) + Layer 2 (for planning/execution). They are the only skills that drive end-to-end workflows.
- When a review skill discovers issues that need fixing, the user loads an orchestrator (e.g., `refactor`) or a Layer 2 skill (e.g., `subagent-driven-development`) separately. Reviews do not dispatch implementation.
- Domain-specific skills are standalone and can be composed with any layer.

### Subsystem Groupings

Named subsystem groupings for scoping reviews and refactors:

| Subsystem | Crates |
|-----------|--------|
| `ir` | kirin-ir |
| `parser` | kirin-chumsky, kirin-derive-chumsky |
| `printer` | kirin-prettyless, kirin-derive-prettyless |
| `interpreter` | kirin-interpreter, kirin-derive-interpreter |
| `derive` | kirin-derive-toolkit, kirin-derive-ir, kirin-derive-chumsky, kirin-derive-interpreter, kirin-derive-prettyless |
| `dialects` | kirin-cf, kirin-scf, kirin-constant, kirin-arith, kirin-bitwise, kirin-cmp, kirin-function |

### Dialect Domain Context

Each dialect crate targets a specific domain. Reviewers (especially the Dialect Author persona) need this context:

| Crate(s) | Domain | Key References |
|-----------|--------|----------------|
| kirin-cf, kirin-scf | Compiler Engineering | Control flow graphs, SSA form, structured control flow (Cytron et al.), dominance, loop nesting |
| kirin-arith, kirin-bitwise, kirin-cmp | Numerics / Arithmetic | Type promotion rules, overflow semantics, IEEE 754, comparison semantics |
| kirin-function | PL / Lambda Calculus | Function application, closures, specialization, parametric polymorphism, calling conventions |
| kirin-constant | Compile-time Evaluation | Constant folding, staged computation, compile-time value semantics |
| kirin-ir (core) | Compiler IR Design | MLIR (Lattner et al. 2020), SSA form, regions/blocks/operations, arena-based IR |
| kirin-interpreter | Abstract Interpretation | Cousot & Cousot framework, lattice-based analysis, widening/narrowing, fixpoint computation |

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
- `kirin-interpreter` — Interpreter traits, `StackInterpreter`, `AbstractInterpreter`
- `kirin-derive-interpreter` — `#[derive(Interpretable, CallSemantics)]`

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

- **Trait decomposition**: The interpreter framework uses three composable sub-traits: `ValueStore` (read/write/write_ssa), `StageAccess<'ir>` (pipeline/active_stage + stage resolution), `BlockEvaluator<'ir>` (eval_block/bind_block_args). `Interpreter<'ir>` is a blanket supertrait auto-implemented for anything implementing `BlockEvaluator<'ir>`. Dialect authors use `I: Interpreter<'ir>`. Custom interpreter developers implement sub-traits individually.

- **`'ir` lifetime pattern**: `StageAccess<'ir>` and `BlockEvaluator<'ir>` are parameterized by `'ir` so that `pipeline()` and `active_stage_info::<L>()` return `&'ir`-lived references. This requires `Self: 'ir` on the traits, which cascades: all type parameters on implementing structs need `'ir` bounds. The `'ir` lifetime is also threaded through `Interpretable<'ir, I>` and `CallSemantics<'ir, I>`. Stage-scoped operations use the `Staged<'a, 'ir, I, L>` builder (constructed via `interp.in_stage::<L>()` or `interp.with_stage(stage)`).

- **`L` on method, not trait**: `Interpretable<'ir, I>` and `CallSemantics<'ir, I>` have `L` as a method-level generic (on `interpret<L>` and `eval_call<L>`), not a trait parameter. This breaks the E0275 cycle that occurred when `L` was on the trait. Impl-level bounds use `InnerType: Interpretable<'ir, I>` (no `L`, no recursion). Method-level bounds (`L: Interpretable<'ir, I> + 'ir`) are resolved coinductively. `#[derive(Interpretable)]` auto-generates the inner-type bounds — no `#[interpret(where(...))]` needed.

- **Stage accessor naming**: `active_stage()` returns `CompileStage` (the stage key), `active_stage_info::<L>()` returns `&'ir StageInfo<L>` (the resolved dialect-specific stage info). Resolve once at the top of a method and pass through to avoid repeated lookups.

- **`ValueStore` is the home for all value read/write operations**: Any function that reads or writes SSA values/results must be a provided method on `ValueStore`, not a standalone function. This includes `write_statement_results` (auto-destructuring products into result slots). Standalone functions that take `&mut impl ValueStore` violate this convention — they should be methods on the trait instead. The reason: it keeps the API surface discoverable and consistent, and allows dialect authors to override behavior if needed.

- **Product types and multi-result**: Multi-result is syntactic sugar over product types. `Continuation::Return(V)` and `Yield(V)` are single-valued — when a function returns multiple values, `V` is a product. The framework auto-destructures products via `ValueStore::write_statement_results`. `ProductValue` is required on `StackInterpreter` because `eval_block` → `run_nested_calls` handles the "hidden unpack" for multi-result calls. Dialect authors who define `Vec<ResultValue>` fields (e.g., `Call`, `Return`, `Yield`) own the multi-value semantics and must require `ProductValue` on their interpret impls. See `docs/design/multi-result-values.md` for the full design.

## Chumsky Parser Conventions

- **Single lifetime `HasParser<'t>`**: All parser traits use a single lifetime `'t` (the input text lifetime). The old two-lifetime system (`HasParser<'tokens, 'src>`) has been collapsed. `HasDialectParser<'t>` has 4 required items: `Output` type, `namespaced_parser`, `clone_output`, `eq_output` — `recursive_parser` has a default impl.

- **`ParseEmit<L>` for text parsing APIs**: `ParseStatementText` and `ParsePipelineText` require `L: ParseEmit<L>`. Three implementation paths: (1) `#[derive(HasParser)]` generates it automatically, (2) implement `SimpleParseEmit` marker for non-recursive dialects to get a blanket impl, (3) implement `ParseEmit` directly for full control. The derive-generated impl delegates to internal `HasParserEmitIR<'t>` (not in the public API) because GAT projection normalization requires a concrete lifetime parameter.

- **`ParseDispatch` for pipeline parsing**: Multi-dialect pipeline parsing uses `ParseDispatch` (a monomorphic dispatch trait) instead of HRTB-based `SupportsStageDispatchMut`. Add `#[derive(ParseDispatch)]` alongside `#[derive(StageMeta)]` on stage enums. Single-dialect pipelines (`Pipeline<StageInfo<L>>`) get a blanket `ParseDispatch` impl. Zero HRTB in the dispatch chain.

- **`#[wraps]` works with Region/Block-containing types**: Dialect types that contain `Region` or `Block` fields (e.g., `Lambda`, `FunctionBody`, SCF operations) can be composed via `#[wraps]` + `HasParser`. See `example/toy-lang/src/language.rs` where `Lexical` (contains `FunctionBody` with Region and `Lambda` with Region) and `StructuredControlFlow` (contains `If`/`For` with Block fields) are both used with `#[wraps]`.

- **`Ctx` default parameter for unified traits**: When the same trait method needs extra context for some implementors (e.g., `CompileStage` for `Pipeline`) but not others (e.g., `StageInfo`), use a default type parameter `Ctx = ()` on the trait. Pair with a blanket `Ext` trait that erases the `()` arg for ergonomic call sites. See `ParseStatementText<L, Ctx>` / `ParseStatementTextExt<L>`.

## Test Conventions

- **Roundtrip tests** (parse → emit → print → compare) go in workspace `tests/roundtrip/<dialect>.rs`
- **Unit tests** for internal logic go inline in the crate (`#[cfg(test)]`)
- **Codegen snapshot tests** go inline in `kirin-derive-chumsky`
- **IR rendering snapshots** go inline in `kirin-prettyless`
- **New test types** (type lattices, values) go in `kirin-test-types`
- **New test dialects** (language enums, stage enums) go in `kirin-test-languages`
- **New test helpers** (roundtrip, parse, fixture builders) go in `kirin-test-utils`
