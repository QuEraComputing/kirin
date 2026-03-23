# kirin-unpack New Crate

**Finding(s):** W10
**Wave:** 3
**Agent role:** Builder
**Estimated effort:** design-work

---

## Issue

A new dialect crate `kirin-unpack` provides DSL-level tuple pack/unpack operations. This is orthogonal to IR multi-result but complements it: a language can use IR multi-result, language-level tuples via kirin-unpack, or both.

The crate provides:
- `MakeTuple` operation: packs multiple SSA values into a single tuple value
- `Unpack` operation: destructures a tuple value into multiple SSA values (multi-result)
- Common `Interpretable` impls for stack interpreter and abstract interpreter
- Standard value type implementations

**Crate(s):** kirin-unpack (new)
**File(s):** all new

**Confidence:** confirmed

## Guiding Principles

- "Dialect developer contract": parser, pretty print, and interpreter are ALL required for dialect authors.
- "No unsafe code": All implementations MUST use safe Rust.
- "Test Conventions": Roundtrip tests go in workspace `tests/roundtrip/<dialect>.rs`. Unit tests go inline.
- "Key Distinction: IR Multi-Result vs Language-Level Tuple": These are different abstraction levels. kirin-unpack operates at the language-level tuple abstraction.
- Use `mod.rs` over `<name>.rs` for modules that contain multiple files.

## Scope

**Files to create:**
| File | Change Type | Description |
|------|-------------|-------------|
| `crates/kirin-unpack/Cargo.toml` | create | Crate manifest |
| `crates/kirin-unpack/src/lib.rs` | create | MakeTuple, Unpack structs with derive macros |
| `crates/kirin-unpack/src/interpret_impl.rs` | create | Interpretable impls |
| `tests/roundtrip/unpack.rs` | create | Roundtrip tests |

**Files to modify:**
| File | Change Type | Description |
|------|-------------|-------------|
| `Cargo.toml` (workspace) | modify | Add kirin-unpack to workspace members |

**Files explicitly out of scope:**
- All existing dialect crates — no modifications
- Abstract interpreter impls — can be added in a follow-up

## Verify Before Implementing

- [ ] **Verify: Wave-1 Continuation supports multi-result**
  Run: `grep -n "SmallVec" crates/kirin-interpreter/src/control.rs`
  Expected: Yield and Return use `SmallVec<[V; 1]>`.

- [ ] **Verify: Wave-0 builder template accepts Vec<ResultValue>**
  Run: `grep -n "cannot be a Vec" crates/kirin-derive-toolkit/src/template/builder_template/helpers.rs`
  Expected: No matches.

- [ ] **Verify: write_results helper exists**
  Run: `grep -rn "pub fn write_results" crates/kirin-interpreter/src/`
  Expected: Helper function exists (added in Wave 1).

- [ ] **Verify: Workspace Cargo.toml structure**
  Run: `grep -n "members" Cargo.toml | head -5`
  Expected: Shows workspace members list for adding kirin-unpack.

## Design Decisions

**Decision 1: Value trait requirements**
- **Primary approach:** `Interpretable` impls are generic over the value type with trait bounds like `TupleValue` (a new trait defining `make_tuple(Vec<V>) -> V` and `unpack(V) -> Result<Vec<V>, InterpreterError>`). Dialect authors implement this trait for their value types.
- **Fallback:** Provide concrete impls only for common value types (e.g., `kirin_test_types::Value`).
- **How to decide:** The trait approach is more general and follows the pattern of `ForLoopValue` in kirin-scf (see `crates/kirin-scf/src/interpret_impl.rs` lines 10-23). Use traits.

**Decision 2: MakeTuple result type**
- **Primary approach:** `MakeTuple` has a single `ResultValue` (it packs N values into 1 tuple). The result type annotation specifies the tuple type.
- **Fallback:** N/A — this is inherent to the pack semantics.

**Decision 3: Unpack results**
- **Primary approach:** `Unpack` has `results: Vec<ResultValue>` (it unpacks 1 tuple into N values). Uses the wave-0 builder template support for Vec<ResultValue>.
- **Fallback:** N/A — this requires the Vec<ResultValue> support from wave-0.

## Implementation Steps

- [ ] **Step 1: Create crate structure**
  Create `crates/kirin-unpack/Cargo.toml` following the same pattern as `crates/kirin-scf/Cargo.toml`:
  ```toml
  [package]
  name = "kirin-unpack"
  version = "0.1.0"
  edition = "2024"

  [dependencies]
  kirin.workspace = true
  kirin-interpreter = { workspace = true }
  smallvec = { workspace = true }

  [dev-dependencies]
  kirin-test-types = { path = "../kirin-test-types" }
  ```
  Add `"crates/kirin-unpack"` to workspace members in root `Cargo.toml`.

- [ ] **Step 2: Define TupleValue trait**
  ```rust
  pub trait TupleValue: Sized {
      fn make_tuple(values: Vec<Self>) -> Self;
      fn unpack(self) -> Result<Vec<Self>, InterpreterError>;
  }
  ```

- [ ] **Step 3: Define MakeTuple operation**
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
  #[chumsky(format = "$make_tuple({args}) -> {result:type}")]
  #[kirin(builders, type = T)]
  pub struct MakeTuple<T: CompileTimeValue> {
      args: Vec<SSAValue>,
      result: ResultValue,
      #[kirin(default)]
      marker: PhantomData<T>,
  }
  ```

- [ ] **Step 4: Define Unpack operation**
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
  #[chumsky(format = "$unpack {source} -> {results:type}")]
  #[kirin(builders, type = T)]
  pub struct Unpack<T: CompileTimeValue> {
      source: SSAValue,
      results: Vec<ResultValue>,
      #[kirin(default)]
      marker: PhantomData<T>,
  }
  ```

- [ ] **Step 5: Implement Interpretable for MakeTuple**
  ```rust
  impl<'ir, I, T> Interpretable<'ir, I> for MakeTuple<T>
  where
      I: Interpreter<'ir>,
      I::Value: TupleValue + Clone,
      T: CompileTimeValue,
  {
      fn interpret<L>(&self, interp: &mut I) -> ... {
          let values: Vec<I::Value> = self.args.iter()
              .map(|ssa| interp.read(*ssa))
              .collect::<Result<_, _>>()?;
          let tuple = TupleValue::make_tuple(values);
          interp.write(self.result, tuple)?;
          Ok(Continuation::Continue)
      }
  }
  ```

- [ ] **Step 6: Implement Interpretable for Unpack**
  ```rust
  impl<'ir, I, T> Interpretable<'ir, I> for Unpack<T>
  where
      I: Interpreter<'ir>,
      I::Value: TupleValue + Clone,
      T: CompileTimeValue,
  {
      fn interpret<L>(&self, interp: &mut I) -> ... {
          let source = interp.read(self.source)?;
          let values = TupleValue::unpack(source).map_err(I::Error::from)?;
          // write_results is the arity-checked helper from kirin-interpreter (added in Wave 1)
          write_results(interp, &self.results, &SmallVec::from(values))?;
          Ok(Continuation::Continue)
      }
  }
  ```

- [ ] **Step 7: Create wrapper enum**
  Note: The trait is named `TupleValue` (Step 2) and the wrapper enum is named `TupleOp` to avoid name collision.
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
  #[wraps]
  #[kirin(builders, type = T)]
  pub enum TupleOp<T: CompileTimeValue> {
      MakeTuple(MakeTuple<T>),
      Unpack(Unpack<T>),
  }
  ```

- [ ] **Step 8: Write unit tests**
  Add inline `#[cfg(test)]` tests for:
  - MakeTuple packs values correctly
  - Unpack destructures values correctly
  - Arity mismatch on Unpack (wrong number of results)

- [ ] **Step 9: Write roundtrip tests**
  In `tests/roundtrip/unpack.rs`:
  - MakeTuple roundtrip
  - Unpack roundtrip
  - Composed: make_tuple then unpack

- [ ] **Step 10: Run all tests**
  Run: `cargo nextest run -p kirin-unpack && cargo nextest run --workspace`
  Expected: All pass.

- [ ] **Step 11: Fix clippy**
  Run: `cargo clippy -p kirin-unpack`
  Expected: No warnings.

## Must Not Do

- Do NOT introduce `#[allow(...)]` annotations — fix root causes.
- Do NOT leave clippy warnings.
- Do NOT implement abstract interpreter support in this plan — defer to follow-up.
- Do NOT modify existing dialects to use kirin-unpack — it's an opt-in dialect.
- No unsafe code.

## Validation

**Final checks:**
```bash
cargo clippy -p kirin-unpack                  # Expected: no warnings
cargo nextest run -p kirin-unpack             # Expected: all tests pass
cargo build --workspace                        # Expected: clean build
cargo nextest run --workspace                  # Expected: no regressions
```

## Success Criteria

1. `kirin-unpack` crate exists with `MakeTuple` and `Unpack` operations.
2. Both operations have parser, pretty print, and interpreter support.
3. `TupleValue` trait provides the extension point for custom value types.
4. Roundtrip tests pass.
5. Arity guardrails work for Unpack (wrong number of results -> error).

**Is this a workaround or a real fix?**
Real fix. New crate providing language-level tuple operations as specified in the design document.
