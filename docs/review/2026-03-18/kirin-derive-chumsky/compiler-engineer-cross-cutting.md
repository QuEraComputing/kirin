# Compiler Engineer — Cross-Cutting Review: kirin-derive-chumsky

## Build Graph

**Dependencies:** `kirin-derive-toolkit`, `kirin-lexer` (with `quote` feature), `chumsky`, `darling`, `indexmap`, `proc-macro2`, `quote`, `syn`.

- **Direct `darling` dependency violates the workspace convention.** The AGENTS.md states: "Derive crates that depend on `kirin-derive-toolkit` must use `kirin_derive_toolkit::prelude::darling` -- never import `darling` directly." However, `kirin-derive-chumsky/Cargo.toml` lists `darling.workspace = true` as a direct dependency. While the workspace currently pins both to `0.23`, this creates a latent risk: if `kirin-derive-toolkit` ever migrates to a newer darling, `kirin-derive-chumsky` would need a separate update. The import in `src/lib.rs` line 19 uses `kirin_derive_toolkit::prelude::darling`, but `src/attrs.rs` line 3 imports `darling::{FromDeriveInput, FromField, FromVariant}` directly from the crate dependency. The `#[darling(...)]` attributes on structs in `attrs.rs` also depend on the direct darling dep. This is a genuine violation -- the `#[derive(FromDeriveInput)]` etc. macros need darling as a direct dependency because they are proc-macro attribute consumers, but the attribute structs could re-derive through the toolkit's darling re-export if the toolkit re-exported the derive macros.

- **`chumsky` is listed as a direct dependency.** This is needed because the codegen references chumsky types and the format string parser likely uses chumsky combinators. However, this means the proc-macro crate compiles chumsky, which is a heavy dependency. The `chumsky` types are only referenced in generated code tokens (as string paths), so the dependency might be avoidable if the codegen only emits path references without actually using chumsky types.

- **`kirin-lexer` with `quote` feature** is needed for the `Token` type in format string parsing and codegen. This is a lightweight dependency.

- **22 source files in `codegen/`** organized into `ast/`, `parser/`, `pretty_print/`, `emit_ir/` subdirectories. Good modular organization.

## Scalability

- **`#[derive(HasParser)]` generates 3 code blocks:** AST definition, `HasDialectParser` impl, and `EmitIR` impl. For an enum with V variants, each block contains match arms or parser alternatives proportional to V. The AST definition creates a parallel enum type, so the generated code roughly doubles the type definitions.

- **The generated AST enum mirrors the source enum** but with parser-specific types (e.g., `SSAValue` becomes a parsed name string). For large enums, this duplication is a compile-time cost since the compiler must type-check both the original and generated enums plus their trait impls.

- **Parser alternatives use `choice()`** which in chumsky is a tuple of parsers. With V variants, the generated `choice()` has V elements. Chumsky's `choice` is implemented via nested tuples up to a certain arity (typically 26), beyond which it requires `choice([vec])`. The derive should handle this limit, though it is unclear if it does. With 50 variants, this would need to be verified.

- **`EmitIR` generates field-by-field emission code.** For each field in each variant, the emit code extracts the parsed value and converts it to the IR representation. This is linear in (variants * fields_per_variant).

- **`PrettyPrint` derive generates one match arm per variant** with format-string-based printing. The generated code for each arm is small (concatenating document fragments). Scales linearly.

## Error Quality

- **Format string validation is thorough.** `validation.rs` checks:
  - All non-default fields are referenced in the format string
  - SSA/Result fields have a name occurrence (not just `:type`)
  - `:name` and `:type` options are only used on SSA-like fields
  - No duplicate default occurrences per field
  - Error messages are specific and actionable, e.g.: "field 'foo' is not mentioned in the format string. All fields must appear... Use {foo} or {foo:name}/{foo:type}..."

- **`parse_derive_input` handles missing `#[kirin(type)]` gracefully.** It first tries to parse normally. If the error is about missing `type`, it injects a placeholder and checks if the input actually needs one. This avoids false positives for value-only types.

- **The `is_missing_type_error` check is fragile.** It checks for the string "Missing field `type`" in the error message. If darling ever changes its error message format, this detection breaks. A more robust approach would be to attempt parsing with an explicit `Option<Type>` and check if the field was present.

- **Errors use `syn::Error` combined with `darling::Error`.** The `validate_format` function accumulates multiple `syn::Error` instances and combines them, so users see all format string errors at once rather than one-at-a-time.

## Compilation Time

- **This is a proc-macro crate, so its compile time is on the critical path.** Every crate that uses `#[derive(HasParser)]` or `#[derive(PrettyPrint)]` must wait for this crate to compile. The dependency on `chumsky` adds significant compile time to this proc-macro.

- **Generated code for `HasDialectParser` includes generic impls** with `'t` lifetime, `I: TokenInput<'t>`, `TypeOutput`, `LanguageOutput` -- 4 generic parameters. The generated `namespaced_parser` method returns `BoxedParser`, which erases the parser type. This bounds the downstream compile-time cost of the generated code.

- **The `GenerateAST`, `GenerateHasDialectParser`, `GenerateEmitIR`, and `GeneratePrettyPrint` generators** each traverse the parsed IR independently. Four traversals of the same data structure could be consolidated into a single pass, but since this runs in the proc-macro at compile time and the data is small (one enum), the overhead is negligible.

- **`ChumskyLayout` and `PrettyPrintLayout`** instantiate `Input<L>` with different associated types, creating two monomorphizations of the input parsing infrastructure. This is unavoidable since the two derives need different attribute types.

## Summary

- **P1** [confirmed] Direct `darling` dependency violates workspace convention; `attrs.rs` imports from `darling` crate directly — `crates/kirin-derive-chumsky/Cargo.toml:14`, `crates/kirin-derive-chumsky/src/attrs.rs:3`
- **P2** [likely] `is_missing_type_error` relies on string matching against darling's error message, which is fragile across darling version upgrades — `crates/kirin-derive-chumsky/src/input.rs:37-39`
- **P2** [uncertain] `chumsky` as a direct proc-macro dependency adds significant compile time; verify whether the dependency is truly needed or if path-only references would suffice — `crates/kirin-derive-chumsky/Cargo.toml:12`
- **P3** [uncertain] Chumsky `choice()` has a max arity for tuple-based alternatives; large enums (>26 variants) may need chunked `choice` generation — generated parser code
