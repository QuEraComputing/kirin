# Multiple-return signatures, signature ownership, and product sequences in the type system

Status: **design agreed; piece 1 is
[#887](https://github.com/QuEraComputing/kirin/pull/887); piece 2 implemented on
`roger/signature-ownership`; piece 3 not started.** Tracks
[#885](https://github.com/QuEraComputing/kirin/issues/885). Piece 1 builds on
[#886](https://github.com/QuEraComputing/kirin/pull/886); piece 3 changes
[#855](https://github.com/QuEraComputing/kirin/pull/855).

## 1. Problem

Statements, `ret`, `yield` and call results already carry several values, and
every engine moves them as a `Product<V>`. `Signature` could only say one
return type, so a function returning several values declared a signature that
did not describe it, and nothing could check a call's result count against its
callee.

Two further problems surfaced while fixing that:

- A specialization's signature is stored twice (on its definition statement
  and on `SpecializedFunctionInfo`) with nothing keeping the copies equal, and
  a definition without a signature field gets an invented
  `() -> <placeholder>`.
- The type system framework (#855) types multiple results as one product
  *type*, which would make every value and abstract domain carry tuples.

## 2. Decisions

1. **A boundary carries a `Product`, and a `Product` is not a type.**
   `kirin_ir::Product<T>` is the framework container for the values crossing a
   call, block or yield boundary. A signature is `Product<T> → Product<T>`:
   one type per parameter, one type per returned value. No SSA value ever has a
   `Product` as its type.
2. **Tuples belong to the language.** A language that has tuples defines its
   own tuple type wrapping a `Product` (`HasProduct` at the type level,
   `HasProductValue` at the value level). `kirin-tuple` is the standard
   provider. Its `new_tuple` and `unpack` statements are the only conversion
   between several values and one tuple value; nothing converts implicitly.
3. **Arity is exact.** `-> A, B` (two returns) and `-> Tuple[A, B]` (one
   tuple-typed return) are different signatures. Binding one result to a
   two-return callee is an error. A language whose multi-return is sugar over
   an explicit tuple keeps a single return and pays nothing.
4. **Text syntax.** The return list is a bare comma list, as call results
   already are: `fn @f(A) -> B, C`. No returns means no arrow: `fn @f(A)`. The
   list is never parenthesized, so a type that prints with parentheses is
   always one return.
5. **Signature ownership.**
   - The staged function owns the *interface* signature. The `stage`
     declaration or the frontend provides it; if a `specialize` appears with no
     `stage` declaration, the interface is initialized from that
     specialization.
   - The definition statement owns its *specialization's* signature (RFC 0004:
     "parsed property, statement-owned"). `SpecializedFunctionInfo` stores no
     signature and reads it through `HasSignature`, as `LinkTarget::body`
     reads the body through `HasCallableBody`.
   - A definition with no signature cannot be a specialization.
     `Signature::placeholder()` is deleted.
6. **A specialization must fit its interface, not equal it.**
   - Checked when the specialization is created: the parameter count and the
     return count equal the interface's.
   - Also checked at creation, by the dialect's signature semantics
     (`Dialect::Signatures`): every parameter and return type is a subset of
     or equal to the interface's. A dialect that declares no order on its
     types gets `ShapeSemantics`, which checks the counts only.
7. **Type system (#855): a `types` sort with sequence variables.** The list of
   types at a boundary is a term of a new sort `types`, distinct from `type`.
   Variables of sort `types` give polymorphism over parameter and return
   counts. Pack slots (one generic body for every arity) are deferred.
8. **Engines do not read signatures.** Arity at run time comes from `ret`
   operands and call result slots; a mismatch is a `ProductArityMismatch`.
   This is unchanged.

Rejected: a single result type whose multi-return form is a product type
(ML/Julia style, #855 as first written). It makes `%t = call f()` and
`%a, %b = call f()` both legal against one signature, requires a pair value in
every interpreter and abstract domain, and makes inlining synthesize a tuple
in languages that have none.

## 3. Pieces and order

```
piece 1 (signature shape)  ──►  piece 2 (signature ownership)
        │
        └──────────────────►  piece 3 (#855: types sort, sequence variables)
```

Pieces 2 and 3 are independent of each other; both need piece 1.

## 4. Piece 1: signature shape

One PR, stacked on #886. Implemented on `roger/mult-ret-product`.

| Area | Change |
|---|---|
| `kirin-ir` `Signature` | `params` and `rets` are both `Product<T>`; built by chaining `Signature::new().takes(..).returns(..)` / `.returns_each(..)` / `.given(..)`; `rets_to_string` is a plain comma join |
| `kirin-ir` function records | `return_types()` returns `&Product<L::Type>` |
| `kirin-chumsky` | `return_clause` (optional `-> T, U`) and `return_type_list` (bare, possibly empty list, for `{sig:return}`) replace #886's parenthesized/`None` parser |
| `kirin-derive-chumsky`, `kirin-prettyless` | parse and print the new syntax; no arrow when there are no returns |
| `kirin-function` | `CallBuilder::returns` (from #886) types call results |
| Fixtures | `demand_boundary.rs` declares the arity it returns |
| Tests | `example/toy-lang/src/interpreter/multi_return_tests.rs`: concrete, constprop, demand, classic liveness, and the arity error |
| Docs | RFC 0004 syntax; `AGENTS.md` "Products and multi-result"; `HasProduct` documented as the language tuple-type opt-in |

Known wart, left as is: with the split form `-> {sig:return}` the arrow is a
literal in the dialect's format string, so a no-return function prints
`fn @f(A) -> cfg {`. Fixing it needs optional sections to accept signature
projections.

Breaking changes to announce in the PR:

- **#886 (JonhasA):** `-> (T, U)` becomes `-> T, U`; `-> None` becomes no
  arrow; `rets()` returns `&Product<T>`.
- **#855 (planza-quera):** `Signature` is now `Product → Product`; see piece 3.
- **Rewriter work (alexcgatea):** `Signature::params()` returns `&Product<T>`
  (iteration, indexing and `len` are unchanged); piece 2 will change the
  `specialize()` builder.

## 5. Piece 2: signature ownership

One PR after piece 1. Implemented on `roger/signature-ownership`.

1. `HasSignature::signature` returns `Option<&Signature<L::Type>>` instead of a
   clone; the `Dialect` derive is updated.
2. `SpecializedFunctionInfo` drops its `signature` field. With it gone the
   record no longer mentions the dialect, so its type parameter is removed
   (`SpecializedFunctionInfo`, `SpecializeError` and `DuplicateSpecialization`
   are no longer generic). `signature(stage)` and `return_types(stage)` read
   the definition statement and return `None` if it has been removed.
   `all_matching` takes the stage.
3. The `specialize()` builder loses its `signature` argument. It reads the
   definition's signature and returns `UnknownDefinition` when the statement
   is not in the stage, or `MissingSignature` when it declares none.
4. The builder checks shape against the staged interface (decision 6, first
   half) and returns `InterfaceMismatch` otherwise.
5. `staged_function()` takes a required signature and no longer accepts
   pre-built specialization records. `Program::define_function` takes no
   signature: the interface starts as the definition's. The parser's fallback
   and `Signature::placeholder()` are deleted, which removes the
   `L::Type: Placeholder` bound from those paths. Unchecked mutators
   (`definition_mut`, `specializations_mut`, `add_specialization`) are removed.
6. `SignatureSemantics::refines(spec, interface)` is the type-fit relation
   (decision 6, second half). It is separate from `applicable`, which matches
   a call site against a candidate and keeps return types equal.
   `LatticeSemantics` overrides it to allow every parameter and return type to
   be a subset; `ExactSemantics` requires equality.
7. The dialect names its semantics: `Dialect` gains
   `type Signatures: SignatureSemantics<Self::Type>`. The derive sets it from
   `#[kirin(signatures = ...)]` and defaults to the new `ShapeSemantics`
   (counts only for `refines`, exact dispatch), because no production type
   implements `TypeLattice` yet and equality would allow one specialization
   per interface. `specialize()` calls `L::Signatures::refines` and returns
   `NotARefinement`; `all_matching` uses `L::Signatures` and loses its type
   parameter. Hand-written `impl Dialect` blocks must name `Signatures`.
8. Two validated builder operations fill gaps that existed before:
   `invalidate_specialization` (retire a specialization whose replacement has
   a different signature, which is not a `Duplicate`) and
   `add_specialization_backedge` (record a caller found after creation).
9. `signature(stage)` stays an `Option`. `specialize()` accepts an in-block
   definition, which the `Rewriter` can erase or replace; a top-level
   definition it cannot. Keeping a rewritten program valid is left to the
   validator.
10. Tests: `kirin-ir`'s `BuilderDialect` gains a `Define(Signature)` variant;
   `kirin-test-languages`' `SimpleLanguage::Function` gains a signature field
   (text `function (f64) -> f64 cfg { … }`) and opts in to
   `LatticeSemantics`, as the first user of the derive attribute.

Consequences for existing tests:

- A parser test built a definition declaring `() -> ()` and registered it
  under `(()) -> ()`. The two copies could disagree before; now there is one.
- `kirin-liveness`'s callable-root tests built specialization records pointing
  at a non-callable statement and at an unallocated statement to exercise the
  engines' `NotCallable` and `MissingStatement` paths. The builder now rejects
  both, so those tests assert the rejection instead. The engine paths remain
  as defensive checks but are no longer reachable through the builder.
- One fixture declared a two-parameter interface with a one-parameter
  specialization; it now uses two parameters of a different type.

Left for later: a validator pass that re-checks, after rewrites, that every
live specialization's definition still exists and still refines its interface.

## 6. Piece 3: `types` sort and sequence variables in #855

Changes to the type system framework, to be agreed with its author.

**3a. Ground sequences**

- Add `Sort::Types`.
- `StaticTerm::Product` changes from "tuple type with at least two components,
  of sort `type`" to "sequence of any length, of sort `types`". It is kept
  flat and cannot be an SSA value's type.
- `StaticTerm::Function` and `TypeSig` take a sequence on both sides. No
  results is the empty sequence; `Unit` stays an ordinary type.
- The framework defines no tuple type. A language defines one as a constructor
  taking a `types` argument (`Tuple : types → type`); `kirin-tuple` can offer
  the standard one.
- A ground `TypeSig` converts to and from `Signature<T, C>`, with `C` carrying
  quantifiers and guards.

**3b. Sequence variables**

- Variables may have sort `types`; `Forall`, `Bound` and `Free` already carry a
  sort.
- `instantiate` splices a substituted sequence in flat.
- Sort checking rejects more than one sequence variable in a sequence (the
  restriction Python's `TypeVarTuple` has), which keeps matching deterministic.
- Call rule: after instantiation, the result sequence's length equals the
  call's slot count.
- Depends on #855 gaining matching/unification.

First users, both already in the repo:

```
new_tuple : ∀X̄.        (X̄)          ⇒ [Tuple[X̄]]
unpack    : ∀X̄.        (Tuple[X̄])   ⇒ X̄
bind      : ∀C̄, Ā, R̄.  (Fn(C̄, Ā) ⇒ R̄, C̄) ⇒ [Fn(Ā) ⇒ R̄]
```

**3c. Specialization order with variables**

- `SignatureSemantics::Env` carries the substitution from `applicable`.
- Order is pointwise; a fixed-arity candidate is more specific than one with a
  sequence variable.
- Ground signatures keep `LatticeSemantics`.

**3d. Pack slots (deferred)**

A result slot typed by a sequence variable, usable only by forwarding it, and
expanded into real slots when the function is specialized (reusing
`clone_cfg`/`IRMap`). Engines run only specializations, so they never see a
pack. Build it when someone needs one IR body for every arity; until then a
generic function has one body per specialization.

## 7. Effect on other work

| Work | Effect |
|---|---|
| RFC 0002 (multi-result format strings) | Consistent; result types are already per slot. |
| RFC 0004 (signature field) | Syntax updated in piece 1; piece 2 implements its "statement-owned" rule; 3b needs text syntax for a sequence variable in a staged signature. |
| March multi-result / product-type plans | `Product` and `HasProduct` are kept as designed. Making every multi-result an implicit product *value* stays superseded. |
| #873 inlining, #851 cloning | Return operands map one to one onto call result slots; no tuple is ever synthesized. |
| #857 analysis manager, engines, `AnalysisResults` | Unchanged; `response` stays a `Product<V>`. |
| #855 linear types | Each returned value is its own slot, so linearity is checked per slot. |
| Kirin 1.0 Python frontend | Single return with an explicit tuple, which is a one-element `Product`. |
