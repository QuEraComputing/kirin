# kirin-chumsky -- Final Review Report

## High Priority (P0-P1)

### P1-1: Hard dependency on `kirin-prettyless` forces printer compilation on parser-only users
- **File:** `crates/kirin-chumsky/Cargo.toml:10`
- **Reviewer:** Compiler Engineer
- **Confidence:** confirmed
- **Details:** `kirin-prettyless` is a non-optional dependency. Any crate needing only parsing must also compile the printer and its transitive dependencies (`prettyless`, `bon`, optional `bat`, optional `serde`). The dependency is used in production code (`ast/symbols.rs` implements `PrettyPrint` on AST types) and re-exported from `lib.rs`, so feature-gating requires: (1) making the `PrettyPrint` impls on AST types conditional, (2) gating the re-export, and (3) gating the prelude re-export. This is feasible but not trivial.
- **Action:** Gate `kirin-prettyless` behind a `pretty` feature (default-enabled for backwards compat). Conditionally compile `PrettyPrint` impls on AST nodes and the re-exports.

### P1-2: Custom type lattices require ~60-115 lines of manual boilerplate
- **File:** `kirin-arith/src/types/arith_type.rs:78-115` (example)
- **Reviewer:** Physicist
- **Confidence:** confirmed
- **Details:** Defining a simple keyword-based type enum (e.g., `ArithType` with variants `i32`, `i64`, `f32`, `f64`) requires manually implementing `HasParser<'t>`, `PrettyPrint`, `DirectlyParsable`, `Display`, and `Placeholder`. This is the single largest onboarding friction point -- most of this is a mechanical string-to-enum mapping. A `#[derive(HasParser)]` that works on simple type enums (not just dialect statement enums) would eliminate the majority of this boilerplate.
- **Action:** Consider extending `#[derive(HasParser)]` or creating a separate derive for keyword-based type enums. Even a declarative macro would help.

## Medium Priority (P2)

### P2-1: `ParseEmit::parse_and_emit` conflates `EmitError` with `ParseError`
- **File:** `crates/kirin-chumsky/src/traits/parse_emit.rs:39-49`
- **Reviewer:** PL Theorist
- **Confidence:** confirmed
- **Details:** The blanket `ParseEmit` impl wraps `EmitError` into a `ParseError` with a zero span (`0..0`). Downstream consumers cannot distinguish "text didn't parse" from "text parsed but IR couldn't be built." This matters for error reporting quality -- a semantic error during emission (e.g., unresolved SSA name) has different recovery strategies than a syntax error.
- **Action:** Consider a richer error enum that preserves the parse-vs-emit distinction, or at minimum tag wrapped `EmitError`s so callers can distinguish them.

### P2-2: DiGraph/UnGraph emit_with share ~60 lines of identical logic
- **File:** `crates/kirin-chumsky/src/ast/graphs.rs:113-291`
- **Reviewer:** Implementer
- **Confidence:** confirmed
- **Details:** Both `emit_with` methods follow the same 4-5 phase pattern. Phases 1-3 (collect port/capture info, build graph with builder API, register SSAs) are nearly identical. Only phases 4-5 differ (edge vs node separation, different attach methods).
- **Action:** Extract shared port/capture setup and SSA registration into a helper function.

### P2-3: Glob re-exports flatten internal modules into public API
- **File:** `crates/kirin-chumsky/src/lib.rs:60-63`
- **Reviewer:** Compiler Engineer
- **Confidence:** likely
- **Details:** `pub use ast::*`, `pub use function_text::*`, `pub use parsers::*`, `pub use traits::*` export everything from four modules into the crate root. Any addition to these modules becomes a public API change. The curated `prelude` module already exists and is the recommended import path.
- **Action:** Consider replacing glob re-exports with explicit re-exports of key types, or accept this as intentional API surface and document it.

### P2-4: Deep derive-generated error messages when type parameter misses trait impl
- **File:** (no specific file -- affects derive output)
- **Reviewer:** Physicist
- **Confidence:** confirmed
- **Details:** Forgetting `HasParser` on a type parameter (e.g., `PulseType`) produces errors from deep in generated code rather than a clear diagnostic. Similar to the `AsBuildStage` diagnostic hint pattern already implemented for `StageInfo`.
- **Action:** Consider adding diagnostic hint traits (similar to `AsBuildStage`) for common missing-impl scenarios in the parser derive.

## Low Priority (P3)

### P3-1: `EmitContext::resolve_ssa` uses fixed `Result(0)` for forward references
- **File:** `crates/kirin-chumsky/src/traits/emit_ir.rs:114`
- **Reviewer:** PL Theorist
- **Confidence:** confirmed
- **Details:** In relaxed dominance mode, all forward references get `Unresolved(Result(0))` as a placeholder. The `0` index does not correspond to the actual result index, giving `ResolutionInfo::Result(usize)` overloaded semantics. This works because resolution happens by name later, but is confusing to readers.
- **Action:** Consider using a distinct `ResolutionInfo::ForwardRef` variant or documenting the convention.

### P3-2: Three error types without a unified hierarchy
- **Files:** `traits/has_parser.rs:89`, `traits/emit_ir.rs:6`, `function_text/error.rs`
- **Reviewer:** PL Theorist
- **Confidence:** confirmed
- **Details:** `ParseError`, `EmitError`, and `FunctionParseError` are independent types. Related to P2-1.

### P3-3: Port/capture builder loop duplication in graphs
- **File:** `crates/kirin-chumsky/src/ast/graphs.rs:137-143, 237-243`
- **Reviewer:** Implementer
- **Confidence:** confirmed
- **Details:** Identical builder loops for ports and captures appear twice. Subsumed by P2-2.

### P3-4: String clones in `register_ssa` calls
- **File:** `crates/kirin-chumsky/src/ast/graphs.rs:165,167,265,267` and `ast/blocks.rs:153`
- **Reviewer:** Implementer
- **Confidence:** confirmed
- **Details:** `ctx.register_ssa(name.clone(), ssa)` clones name strings because `register_ssa` takes owned `String`. This is a cross-crate API concern (`EmitContext` API).

### P3-5: Format string DSL lacks standalone documentation
- **Reviewer:** Physicist
- **Confidence:** confirmed
- **Details:** The `{.keyword}`, `{field:name}`, `{field:type}` format DSL is only documented through inline examples in existing dialects.

### P3-6: Two-pass pipeline parsing doubles parse-phase cost
- **Reviewer:** Compiler Engineer
- **Confidence:** informational
- **Details:** `first_pass_concrete` and `second_pass_concrete` traverse function bodies twice. Acceptable for correctness.

### P3-7: Missing `#[must_use]` annotations
- **Reviewer:** Implementer
- **Confidence:** confirmed
- **Details:** Zero `#[must_use]` annotations in the crate. Key candidates: `EmitContext::new()`, `EmitContext::lookup_ssa()`, `EmitContext::lookup_block()`. Note: `Result`-returning methods already get implicit `#[must_use]` from the `Result` type.

## Strengths

1. **Single-lifetime `HasParser<'t>` design** is clean and well-scoped. The collapse from the two-lifetime system was the right call. (PL Theorist, Physicist)

2. **Three-tier ParseEmit API** (derive / SimpleParseEmit / manual) provides appropriate gradual complexity. Most users never leave the derive path. (PL Theorist, Physicist)

3. **Format string DSL** (`#[chumsky(format = "...")]`) is readable and low-friction for the common case. The derive handles all parser/AST/EmitIR generation from a single annotation. (Physicist)

4. **Pipeline parsing ergonomics** are excellent -- 2 lines to parse a file into a pipeline. Statement parsing is 2 concepts. (Physicist)

5. **ParseDispatch monomorphic design** correctly eliminates HRTB from the dispatch chain. Scales linearly and composes cleanly. (Compiler Engineer, PL Theorist)

6. **Witness methods** (`clone_output`/`eq_output`) are a well-motivated dictionary-passing technique that avoids GAT E0275 issues. (PL Theorist)

7. **Explicit recursive parser handle** is the principled approach corresponding to the `fix` operator, preventing parser construction cycles. (PL Theorist)

8. **`strsim` typo detection** in stage name resolution is a thoughtful touch for error quality. (Compiler Engineer)

## Filtered Findings

| Finding | Reviewer | Reason for filtering |
|---------|----------|---------------------|
| Three ParseEmit paths create confusion | Physicist (P3) | Intentional design per AGENTS.md. Three paths are documented as gradual complexity. Most users use derive only. |
| `HasDialectParser` 4 required items are complex | (not raised, but preemptive) | Intentional per design context. Users never implement manually. |
| `BoxedParser` type-erasure overhead | PL Theorist (informational) | Pragmatic and intentional. Parsing is not the bottleneck. |
| Composite parser nested tuple types stress type checker | Compiler Engineer (P3) | Inherent to chumsky combinator approach. No actionable fix without changing parser library. |
| `emit_with` + `EmitIR` wrapper pattern is boilerplate | Implementer | Not actionable -- pattern enables `#[wraps]` dialect types to intercept statement emission. Intentional design. |

## Suggested Follow-Up Actions

1. **[P1-2] Prototype a type-enum derive** -- Start with a simple `#[derive(HasParser)]` for keyword-based type enums (like `ArithType`). This would have the highest impact on new-user onboarding. Even a declarative `keyword_enum!` macro would help.

2. **[P1-1] Feature-gate `kirin-prettyless`** -- Add a `pretty` feature (default-enabled) that gates the prettyless dependency, AST `PrettyPrint` impls, and re-exports. This unblocks parse-only downstream crates.

3. **[P2-2] Extract graph emit helper** -- Refactor `DiGraph`/`UnGraph` `emit_with` to share port setup and SSA registration logic. Estimated ~60 lines saved.

4. **[P2-1] Preserve error origin in `ParseEmit`** -- Either change the return type to distinguish parse errors from emit errors, or add a tag/variant to `ParseError` indicating the error source.

5. **[P3-1, P3-5] Documentation pass** -- Document the format string DSL syntax in a standalone section, and add a comment explaining the `Result(0)` convention for forward references.
