# API Examples Walkthrough

This directory walks through the complete API surface using concrete examples, progressing
from simple pure dialects to complex composed dialects with machine state and custom seeds.

## Examples

1. [Pure Dialect (Arithmetic)](01_pure_arith.md) — simplest case: no machine, no errors, base effects only
2. [Control Flow (Branch)](02_control_flow.md) — base effects for jumps and error returns
3. [Function Call (Seed)](03_function_call.md) — executing a seed directly via `&mut I`
4. [SCF If (Custom Seed)](04_scf_if.md) — custom seed composing BlockSeed
5. [SCF For Loop (Seed Composition)](05_scf_for.md) — looping seed with carried state
6. [Stateful Dialect (Direct Mutation)](06_stateful_direct.md) — machine mutation via ProjectMut
7. [Stateful Dialect (Machine Effects)](07_stateful_effects.md) — effects through consume_effect pipeline
8. [Composed Dialect (Homogeneous)](08_composed_simple.md) — all sub-dialects return base effects
9. [Composed Dialect (Mixed Effects)](09_composed_mixed.md) — sub-dialects with different effect types
10. [Error Propagation](10_error_propagation.md) — custom errors, InterpreterError, error flow

## API Surface Summary

| Dialect type | Effect | Error | Needs ProjectMut | Uses Seeds |
|---|---|---|---|---|
| Pure (arith, cmp) | `()` | `Infallible` | No | No |
| Control flow (cf) | `()` | `Infallible` | No | No |
| Checked (overflow) | `()` | Custom error | No | No |
| Calls (function) | `()` | `Infallible` | No | Yes (FunctionSeed) |
| SCF (if, for) | `()` | `Infallible` | No | Yes (custom seeds) |
| Stateful (memory) | `()` or machine effect | `Infallible` | Yes | No |
| Composed | `()` or composed DE | `Infallible` or composed ME | Depends | Depends |
