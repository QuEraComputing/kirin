# Derive Support and Pilot Dialect Adoption

**Wave:** 4
**Agent role:** Implementer
**Estimated effort:** large

---

## Issue

Once the v2 runtime is real, downstream crates need a practical way to target
it. This wave covers two connected tasks:

1. update `kirin-derive-interpreter` so it can generate code for the new trait
   family and effect protocol, and
2. prove the API on real dialect crates by adding v2 interpretation support to
   a pilot set.

The pilot set should be:

- `kirin-function`
- `kirin-scf`
- `kirin-cf`
- `example/toy-lang`

`kirin-cf` is included because region execution and structured control flow are
hard to validate in practice without real branch semantics.

## Scope

**Files to add or modify:**

- `crates/kirin-derive-interpreter/src/*`
- `crates/kirin-interpreter-2/Cargo.toml`
- `crates/kirin-interpreter-2/src/lib.rs`
- `crates/kirin-function/src/*`
- `crates/kirin-scf/src/*`
- `crates/kirin-cf/src/*`
- `example/toy-lang/src/*`
- new or updated tests in the pilot crates

**Out of scope:**

- removing `kirin-interpreter`,
- making `kirin-interpreter-2` the workspace default,
- broad migration of every dialect crate.

## Adoption Strategy

Keep the old and new runtimes side by side. The pilot crates should gain
v2-specific interpretation modules rather than rewriting their current runtime
integration in place.

Recommended pattern:

- keep old impls in existing modules,
- add `interpret_v2.rs` or similarly named sibling modules,
- use explicit crate paths to avoid trait-name ambiguity during the dual-runtime
  period.

## Derive Work

Treat derive support as a real API migration:

- `Interpretable` derive must emit the new `ExecEffect` return type.
- The old `CallSemantics` / `SSACFGRegion` derives need either new v2-focused
  derives or compatibility expansions that can target `CallableBody` and the
  new execution model.
- Crate-path override support must work for `::kirin_interpreter_2`, not just
  `::kirin_interpreter`.

Do not assume this is a path-only rewrite. The generated bounds and return types
must change materially.

## Implementation Steps

- [ ] Extend `kirin-derive-interpreter` to support the new crate path and v2
  trait contracts.
- [ ] Decide whether the new derive names should be `CallableBody` /
  `SSACFGRegion` / compatibility wrappers, and keep that choice consistent
  across docs and codegen.
- [ ] Add v2 interpretation support to `kirin-function`, `kirin-scf`, and
  `kirin-cf`, keeping the old interpreter integration intact.
- [ ] Wire `example/toy-lang` to run with the new interpreter as an explicit
  path or feature, not as a silent replacement.
- [ ] Add integration tests that exercise real function calls, structured
  control flow, and branch behavior through `kirin-interpreter-2`.

## Validation

Run:

```bash
cargo build -p kirin-derive-interpreter
cargo nextest run -p kirin-function -p kirin-scf -p kirin-cf -p toy-lang
cargo nextest run -p kirin-interpreter-2
```

## Success Criteria

1. Real dialect crates can target `kirin-interpreter-2` without deleting their
   old interpreter support.
2. The derive crate can generate correct v2 code rather than hardcoding old
   continuation-era assumptions.
3. `example/toy-lang` can exercise the new runtime on nontrivial programs.
4. The workspace has a credible pilot migration path before any default switch
   is considered.
