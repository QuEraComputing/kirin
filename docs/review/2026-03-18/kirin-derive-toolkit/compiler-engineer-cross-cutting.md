# Compiler Engineer — Cross-Cutting Review: kirin-derive-toolkit

## Build Graph

**Dependencies:** `darling`, `indexmap`, `manyhow`, `proc-macro2`, `quote`, `syn`.

- **This crate is not a proc-macro crate itself** -- it is a regular library that provides shared infrastructure for proc-macro crates (`kirin-derive-ir`, `kirin-derive-chumsky`, `kirin-derive-interpreter`, `kirin-derive-prettyless`). This is the correct pattern: factoring shared derive logic into a non-proc-macro library allows code reuse without the constraints of proc-macro crate rules.

- **`darling 0.23` is the canonical version** for Kirin derive macros. The re-export from the prelude (`pub use darling`) is critical for the workspace's darling version discipline. All downstream derive crates should import darling from here.

- **`manyhow` is used for error handling in proc-macro code.** It provides the `darling` feature integration. This is a small crate with minimal impact.

- **`indexmap` is used for ordered maps** in the IR representation. It preserves insertion order for field iteration, which is important for format-string-order field processing.

- **No runtime dependencies.** This crate is only used at compile time by proc-macro crates. It does not appear in any runtime dependency graph. Good.

- **Dev-dependencies:** `insta` for snapshot testing, `kirin-test-utils` for test utilities. Minimal.

## Scalability

- **`Input<L>::compose()` builds a `TemplateBuilder`** that accumulates templates via `.add()`. Each template generates one or more trait impls. The `#[derive(Dialect)]` macro adds 21 templates (14 field iters + 5 properties + 1 builder + 1 marker). For each template, the builder iterates over all variants to generate match arms. With V variants and T templates, the total generated code is O(V * T) tokens. At V=50, T=21, that is 1050 match arms per `#[derive(Dialect)]` invocation.

- **`TraitImplTemplate::field_iter` generates a chained iterator** for field iteration. For enums, each variant arm produces either a field reference or is skipped. The generated code is proportional to (variants * fields_per_variant), which is acceptable.

- **`TraitImplTemplate::bool_property` generates a match with one arm per variant.** With N variants, each property trait generates N match arms. For `IsTerminator`, only variants marked `#[kirin(terminator)]` return `true`; others return `false`. The code is simple per-arm.

- **`BuilderTemplate` generates a constructor function** with one branch per variant (for enums). The constructor body includes field default resolution and `ResultValue` placeholder generation. This is linear in variant count.

- **`DeriveContext` pre-computes `StatementContext` for each variant**, including wrapper type/binding and match pattern. This pre-computation avoids redundant work across multiple templates, which is a good optimization for large enums.

## Error Quality

- **Errors use `darling::Error` and `syn::Error` consistently.** The `darling::Error::custom()` pattern with `.with_span()` produces compile errors that point to the offending source location. This is the standard approach for derive macro diagnostics.

- **`parse_stage_variants` produces specific error messages:**
  - "stage derive macros can only be applied to enums"
  - "each variant must be a single-field tuple, e.g. `Variant(StageInfo<L>)`"
  - "field type must be `StageInfo<L>` where L is a dialect type"
  - "missing `#[stage(name = \"...\")]` attribute"
  - "stage enum requires at least one variant"

  Each error uses `syn::Error::new_spanned` with the relevant AST node, so the error points to the correct location.

- **`skip_meta_value` handles unknown attributes gracefully.** When parsing `#[stage(...)]`, unknown keys are silently consumed rather than producing errors. This allows multiple derive macros to share the same attribute namespace without conflicting. Good composability.

- **`Input::from_derive_input` rejects unions** with a clear message: "Kirin ASTs can only be derived for structs or enums."

- **Missing: validation for duplicate variant names.** If an enum has two variants with the same name (impossible in Rust, so this is academic), or two `#[stage(name = "...")]` with the same name, there is no explicit duplicate detection. The latter could cause silent stage resolution failures at runtime.

## Compilation Time

- **`Input<L>` is parameterized by `Layout`**, which carries 4 associated types (`StatementExtra`, `ExtraGlobalAttrs`, `ExtraStatementAttrs`, `ExtraFieldAttrs`). Each derive macro that uses `Input<L>` instantiates these types. With 4 derive crates each using a different `Layout`, that is 4 monomorphizations of `Input` and all its methods. Since these are compile-time-only, the cost is proc-macro execution time, not downstream build time.

- **The template system uses closures** (`Custom::new(|ctx, stmt_ctx| { ... })`) which are monomorphized per call site. This is fine since each closure is used once.

- **`quote!` macro usage is pervasive.** Every token generation call invokes `quote!`, which expands to a series of `TokenStream::extend` calls. For large generated code (21 trait impls), this is the dominant cost in proc-macro execution. The proc-macro is compiled once and runs multiple times (once per `#[derive(Dialect)]` invocation), so the execution time scales with (invocations * variants * templates).

- **`syn` full features are enabled workspace-wide** (`features = ["extra-traits", "full"]`). The `extra-traits` feature adds `Debug`, `Eq`, `Hash` impls for all AST types, which increases `syn`'s own compile time. If `extra-traits` is only needed for testing/debugging, it could be made conditional. However, since `darling` likely requires it, this may not be avoidable.

## Summary

- **P3** [uncertain] No duplicate detection for `#[stage(name = "...")]` values; two variants with the same stage name would cause silent runtime failures — `crates/kirin-derive-toolkit/src/stage_info.rs:13-163`
- **P3** [informational] 21 templates per `#[derive(Dialect)]` generates O(V * 21) match arms; at 50 variants this is ~1050 arms of generated code — `crates/kirin-derive-ir/src/generate.rs:238-267`
- **P3** [informational] `syn` full + extra-traits features are always enabled, adding baseline compile cost to all proc-macro crates — workspace `Cargo.toml:39`
