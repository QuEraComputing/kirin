# Multiple-return signatures, signature ownership, and product sequences in the type system

Status: **design agreed; piece 1 implemented on `roger/mult-ret-product`
(uncommitted), pieces 2 and 3 not started.** Tracks
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
   - Checked by the signature semantics / type checker: every parameter and
     return type is a subset of or equal to the interface's.
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
| `kirin-ir` `Signature` | `params` and `rets` are both `Product<T>`; `new`/`with_rets` accept any iterator; `rets_to_string` is a plain comma join |
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

One PR after piece 1.

1. `HasSignature::signature` returns `Option<&Signature<L::Type>>` instead of a
   clone; update the `Dialect` derive.
2. `SpecializedFunctionInfo` drops its `signature` field. `signature(stage)`
   reads the definition statement. Update serde, `all_matching`, the printer
   and the duplicate-specialization check.
3. The `specialize()` builder loses its `signature` argument. It reads the
   definition's signature and returns a new `MissingSignature` error when there
   is none.
4. The builder checks shape against the staged interface (decision 6, first
   half) and returns an `InterfaceMismatch` error otherwise.
5. `staged_function()` takes a required signature. The parser's fallback and
   `Signature::placeholder()` are deleted, which also removes the
   `L::Type: Placeholder` bound from those paths.
6. Relax `LatticeSemantics::applicable` so return types may be a subset of the
   candidate's, not only equal (decision 6, second half).
7. Tests: the `kirin-ir` builder tests use plain statements as definitions and
   pass signatures separately; give them a test definition with a signature
   field (in `kirin-test-languages`).

Open within this piece: where the type-fit half of decision 6 runs. Nothing
fixes which `SignatureSemantics` a stage uses, and `all_matching` has no caller
outside tests. Either the stage records its semantics, or the check stays with
the type checker from piece 3.

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
