# Physicist -- Ergonomics/DX Review: kirin-interpreter

## Repetition & Boilerplate

### 1. Interpreter trait bound boilerplate on manual Interpretable impls

Every manual `Interpretable` impl requires the same 5-line method signature:
```rust
fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Interpretable<'ir, I> + 'ir,
```

This is repeated verbatim across every dialect's `interpret_impl.rs`. See:
- `kirin-arith/src/interpret_impl.rs:31-35`
- `kirin-function/src/interpret_impl.rs:29-33`, `65-69`, `103-107`, `123-127`, `137-143`, `256-260`
- `kirin-cf` (implied, same pattern)

The where clause is identical every time. A macro or default could help, though the `L` on method design is intentional.

### 2. Match arm repetition in wrapper enum Interpretable impls

When manually implementing `Interpretable` for a wrapper enum like `Lexical<T>` or `Lifted<T>`, every variant needs a match arm that just delegates `op.interpret::<L>(interp)`. See `kirin-function/src/interpret_impl.rs:109-114`:
```rust
match self {
    Lexical::FunctionBody(op) => op.interpret::<L>(interp),
    Lexical::Lambda(op) => op.interpret::<L>(interp),
    Lexical::Call(op) => op.interpret::<L>(interp),
    Lexical::Return(op) => op.interpret::<L>(interp),
}
```
The `#[derive(Interpretable)]` macro eliminates this for the top-level language enum. But inner dialect enums like `Lexical<T>` do this manually. Consider making `#[derive(Interpretable)]` work for inner dialect enums too (it may already -- the toy-lang `HighLevel` uses it).

### 3. SSACFGRegion boilerplate for wrapper enums

Similarly, `SSACFGRegion` for `Lexical<T>` (line 81-95) and `Lifted<T>` (line 267-280) manually delegates `entry_block` per callable variant and returns error for non-callable ones. `#[derive(SSACFGRegion)]` should handle this if `#[callable]` is marked.

## Lifetime Complexity

### `'ir` lifetime threading

The `'ir` lifetime appears on:
- `Interpretable<'ir, I>` -- the trait itself
- `Interpreter<'ir>` -- the blanket supertrait
- `StageAccess<'ir>` -- stage resolution
- `BlockEvaluator<'ir>` -- block execution
- `CallSemantics<'ir, I>` -- function call dispatch
- `Staged<'a, 'ir, I, L>` -- the stage-scoped builder
- `StackInterpreter<'ir, V, S, E, G>` -- the concrete interpreter

For a user writing a manual `Interpretable` impl, they must write `impl<'ir, I, T>` and thread `'ir` through. This is unavoidable given the design constraints, but it means every interpreter-related impl has an extra lifetime parameter compared to what a physicist would expect.

### Staged<'a, 'ir, I, L> has two lifetimes

`Staged` has two lifetime parameters plus two type parameters. Users encounter this when calling `interp.in_stage::<HighLevel>().call(spec, &args)`. In practice, lifetime inference handles this and users never write the type explicitly -- this is well-designed.

### StackInterpreter type parameter explosion

`StackInterpreter<'ir, V, S, E, G>` has 5 type/lifetime parameters. The common case in toy-lang is:
```rust
let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
```
The `_` for the stage type and defaults for `E` and `G` help, but the turbofish `StackInterpreter<i64, _>` is still needed because Rust can't infer `V` from context alone. This is a reasonable trade-off.

## Concept Budget

### Use Case: "Write an interpreter pass for my dialect (3 operations)"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `Interpretable<'ir, I>` trait | kirin-interpreter | Medium |
| `Interpreter<'ir>` bound | kirin-interpreter | Low (just use as bound) |
| `Continuation` enum | kirin-interpreter | Medium (Continue, Jump, Call, Return, Yield, Fork, Ext) |
| `ValueStore::read/write` | kirin-interpreter | Low |
| `I::Value` associated type | kirin-interpreter | Low |
| `I::Error` + `From<InterpreterError>` | kirin-interpreter | Low |
| `'ir` lifetime | kirin-interpreter | Medium (why is it needed?) |
| `L` type parameter on method | kirin-interpreter | Medium (why is it needed?) |
| `HasStageInfo<L>` bound | kirin-ir | Low |
| `SSACFGRegion` (if callable) | kirin-interpreter | Medium |

**Total: ~10 concepts.** The high-complexity items (`Continuation` variants, `'ir` purpose, `L` on method) would benefit from a tutorial/guide.

### Use Case: "Run my dialect through the stack interpreter"

| Concept | Where learned | Complexity |
|---------|--------------|------------|
| `Pipeline<S>` | kirin-ir | Medium |
| `StackInterpreter::new` | kirin-interpreter | Low |
| `interp.in_stage::<L>()` | kirin-interpreter | Low |
| `Staged::call` | kirin-interpreter | Low |
| `SpecializedFunction` resolution | kirin-ir | High (6+ lookups) |
| `CompileStage` | kirin-ir | Low |

**Total: ~6 concepts.** But the `SpecializedFunction` resolution is the main pain point (see kirin-ir review).

## Toy Scenario Results

### Scenario: Implement Interpretable for TweezerPulse::Ramp

```rust
impl<'ir, I> Interpretable<'ir, I> for TweezerPulse
where
    I: Interpreter<'ir>,
    I::Value: Into<f64> + From<f64>,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            TweezerPulse::Ramp { start, end, duration, result } => {
                let s: f64 = interp.read(*start)?.into();
                let e: f64 = interp.read(*end)?.into();
                let d: f64 = interp.read(*duration)?.into();
                // compute ramp parameters...
                interp.write(*result, (s + e * d).into())?;
                Ok(Continuation::Continue)
            }
            _ => unreachable!(),
        }
    }
}
```

**What worked well:**
- `interp.read(ssa)` and `interp.write(result, value)` are clean and obvious
- `Continuation::Continue` is self-documenting
- The pattern (read inputs, compute, write output, return Continue) is simple and repeatable

**What was confusing:**
- The where clause is a wall of text I copied from another impl without understanding
- `L: Interpretable<'ir, I> + 'ir` -- why does my operation need to know about `L`? (Answer: for nested dialect dispatch, but as a leaf operation author I never use `L`)
- The `_ => unreachable!()` for `__Phantom` is ugly

### Scenario: Run the interpreter on a parsed pipeline

Looking at toy-lang's `main.rs:119-141`, the usage is clean once you have the `SpecializedFunction`:
```rust
let mut interp: StackInterpreter<i64, _> = StackInterpreter::new(&pipeline, stage_id);
let result = interp.in_stage::<HighLevel>().call(spec, &args)?;
```
This is excellent -- two lines to create and run. The problem is the 50+ lines needed to get `spec` (function resolution, covered in kirin-ir review).

## Summary

- [P2] [confirmed] Identical where clause boilerplate on every manual Interpretable impl (5 lines, always the same). `kirin-arith/src/interpret_impl.rs:31-35`
- [P2] [confirmed] `_ => unreachable!()` match arm needed for `__Phantom` variant in manual impls. `kirin-arith/src/interpret_impl.rs:91`
- [P1] [confirmed] Inner dialect wrapper enums (Lexical, Lifted) require manual Interpretable + SSACFGRegion delegation that could be derived. `kirin-function/src/interpret_impl.rs:81-116`
- [P3] [confirmed] `L` type parameter on `interpret` method is confusing for leaf operation authors who never use it. Design is intentional (breaks cycles) but needs better docs.
- [P3] [confirmed] `Continuation` has 7+ variants; leaf dialect authors only use 2-3 (Continue, Return, Jump). A guide showing "which variant for which pattern" would help.
- [P2] [likely] No convenience method to go from function name + stage to a callable `SpecializedFunction`. The 6+ step lookup dance is repeated in every program that runs the interpreter.
