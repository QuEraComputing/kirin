# U5: Output & Dialects -- Formalism Review

## Findings

### [P2] [likely] `PrettyPrint` requires `L: PrettyPrint` recursively but provides no termination guarantee -- traits.rs:41

The `PrettyPrint` trait's method signature requires `L: Dialect + PrettyPrint`, creating a recursive constraint: to pretty-print a value, the entire dialect must itself implement `PrettyPrint`. This is sound in practice because Rust's trait coherence ensures a single canonical impl, and the derive macro generates non-recursive implementations for leaf types. However, the recursive bound means that *any* type implementing `PrettyPrint` implicitly requires the entire dialect closure to be pretty-printable -- there is no way to pretty-print a fragment of a dialect without the whole dialect being resolved.

This is a consequence of the design choice to have `Document<'a, L>` carry the full dialect type, which is needed to print nested blocks/regions. The alternative would be a trait-object-based approach where `Document` is not parameterized by `L`, but that would sacrifice static dispatch.

**Alternative formalisms:**

| Approach | Static dispatch | Incremental printing | Bound complexity |
|----------|----------------|---------------------|-----------------|
| `L: PrettyPrint` on method (current) | Yes | No (all-or-nothing) | O(1) recursive bound |
| `dyn PrettyPrint` document | No | Yes (per-node) | O(1) no recursive |
| Two-phase: collect + render | Yes | Yes | O(n) phases |

**Suggested action:** No change needed. The recursive `L: PrettyPrint` bound is the standard encoding for a mutually recursive pretty-printer over a closed type family. The bound complexity is O(1) because each dialect's derive generates a single monomorphic impl.

**References:** Hinze, "Generics for the masses" (type-indexed functions over recursive types); McBride, "Elimination with a motive" (recursive elimination principles).

### [P2] [uncertain] `Lexical` vs `Lifted` are isomorphic modulo one variant substitution -- kirin-function/src/lib.rs:39,49

`Lexical<T>` = `{FunctionBody, Lambda, Call, Return}` and `Lifted<T>` = `{FunctionBody, Bind, Call, Return}`. These differ in exactly one variant (`Lambda` vs `Bind`). From a type-theoretic perspective, both are coproducts sharing 3 of 4 summands. The `#[wraps]` derive handles this correctly, but the two enums duplicate the `FunctionBody`, `Call`, `Return` wrapping boilerplate.

A more compositional encoding would use row polymorphism or variant composition: `type Lexical<T> = Common<T> + Lambda<T>` and `type Lifted<T> = Common<T> + Bind<T>`. Rust does not support row polymorphism natively, but the `#[wraps]` mechanism already provides a form of it.

**Alternative formalisms:**

| Approach | Dedup | Extensibility | Rust ergonomics |
|----------|-------|---------------|-----------------|
| Two enums (current) | None | Fixed 2 variants | Good (`#[wraps]` handles it) |
| `Common<T>` + mode-specific wrapper | Shared arms | Easy new modes | Nested match |
| Generic `Function<T, Mode>` with mode phantom | Full dedup | Open | Complex bounds |

**Suggested action:** The current design is acceptable given that only two modes exist. If a third mode is added (e.g., CPS-converted), consider extracting `Common<T>` as a shared sub-enum with `#[wraps]` delegation.

**References:** Remy, "Type inference for records in a natural extension of ML" (row polymorphism); Garrigue, "Programming with polymorphic variants" (OCaml variant composition).

## Strengths

- The `RenderDispatch` trait provides clean type-erased rendering for heterogeneous pipeline stages, following the standard visitor/double-dispatch pattern. The blanket impl for `StageInfo<L>` ensures zero boilerplate for single-dialect pipelines.
- The `PipelineDocument` two-level rendering (iterate functions, then iterate stages per function) correctly models the product structure of the pipeline's function-stage matrix.
- `FunctionBody<T>` with `Signature<T>` field and derive-generated `HasSignature` cleanly separates the structural (Region) from the semantic (type signature) aspects of a function, and the format string `"fn {:name}{sig} {body}"` compactly encodes both parser and printer from a single specification.
