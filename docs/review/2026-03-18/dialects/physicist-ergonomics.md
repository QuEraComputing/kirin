# Dialects -- Physicist (Ergonomics/DX) Review

**Crates:** kirin-arith, kirin-bitwise, kirin-cmp, kirin-cf, kirin-scf, kirin-constant, kirin-function
**Lines:** ~3295

## Scenario: "I want to add a new dialect similar to kirin-arith"

The dialect definition path is excellent. `Arith<T>` (lib.rs:84-129) shows the ideal: a single enum with `#[derive(Dialect, HasParser, PrettyPrint)]`, format strings on each variant, and zero lifetime annotations. The concept budget for *defining* a dialect is low. However, once you need a custom type lattice (ArithType), the boilerplate surfaces -- this is already tracked as P1-6.

## Concept Budget: New Dialect

| Concept | Required? | Where learned |
|---------|-----------|---------------|
| `Dialect` derive | Yes | lib.rs |
| `#[kirin(type = T)]` | Yes | lib.rs |
| `#[chumsky(format = ...)]` format DSL | Yes | lib.rs |
| `CompileTimeValue` bound | Yes | lib.rs |
| `SSAValue`, `ResultValue`, `Successor`, `Block`, `Region` | Yes (field types) | prelude |
| `PhantomData` + `__Phantom` variant | Yes (generic) | lib.rs |
| `#[kirin(pure/speculatable/terminator)]` | Per-variant | lib.rs |
| `#[wraps]` for composition | For wrapper enums | kirin-scf |

**Total: 8 concepts** for a basic dialect definition. This is good.

## Findings

### D1. Interpreter where-clause boilerplate is repetitive across all 7 dialects (P2, high confidence)

Every `Interpretable` impl repeats the same 5-line method-level where clause. Across 7 crates, there are ~14 manual `Interpretable` impls (arith, bitwise, cmp, cf, constant, function x 7, scf x 4). Each carries the identical:
```
I::StageInfo: HasStageInfo<L>,
I::Error: From<InterpreterError>,
L: Interpretable<'ir, I> + 'ir,
```
This is the method-signature tax of L-on-method. Already tracked as P2-H but seeing it repeated across all 7 dialects reinforces the priority. The *impl*-level bounds are appropriately varied (e.g., `I::Value: Add + Sub + ...` for arith).

**Files:** `kirin-arith/src/interpret_impl.rs:31-36`, `kirin-cf/src/interpret_impl.rs:15-19`, `kirin-bitwise/src/interpret_impl.rs:20-24`, `kirin-cmp/src/interpret_impl.rs:229-233`, `kirin-scf/src/interpret_impl.rs:165-169` (x4 impls), `kirin-function/src/interpret_impl.rs:29-33` (x7 impls).

### D2. Manual `Interpretable` delegation on inner dialect enums (P1, high confidence)

`StructuredControlFlow` (scf interpret_impl.rs:229-247), `Lexical` (function interpret_impl.rs:97-116), and `Lifted` (function interpret_impl.rs:282-301) all manually delegate `interpret` to inner variants. This is exactly what `#[derive(Interpretable)]` does for top-level language enums. Already tracked as P1-8 -- confirming the impact is 3 enums, ~69 lines.

### D3. `ArithValue` manual PartialEq/Hash/Display is ~100 lines of mechanical code (P3, medium confidence)

`kirin-arith/src/types/arith_value.rs:51-157` contains 3 manual trait impls that are purely mechanical match arms. A macro_rules could reduce this, though the f32/f64 special casing (to_bits) makes it non-trivial. Low priority since ArithValue is a one-time cost.

### D4. Dialect patterns are highly consistent (strength)

All 7 dialects follow the same structure: lib.rs defines the enum, interpret_impl.rs behind `#[cfg(feature = "interpret")]`, tests.rs behind `#[cfg(test)]`. The format string DSL eliminates parser/printer code entirely for operations. This consistency means learning one dialect teaches all.
