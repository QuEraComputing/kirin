# API Examples Walkthrough

These examples illustrate the effect-first contract from simplest to most complex cases.

## Examples

1. [Pure Dialect (Arithmetic)](01_pure_arith.md) — no machine effects, no custom errors
2. [Control Flow (Branch)](02_control_flow.md) — jumps and interpreter-shell errors
3. [Function Call (Seed)](03_function_call.md) — reusable function/body entrypoint via `FunctionSeed`
4. [SCF If (Inline Orchestration)](04_scf_if.md) — inline control logic over `BlockSeed`
5. [SCF For Loop (Inline Loop)](05_scf_for.md) — inline loop orchestration over `BlockSeed`
6. [Rejected: Direct Mutation](06_stateful_direct.md) — why dialect semantics cannot use `ProjectMut`
7. [Stateful Dialect (Machine Effects)](07_stateful_effects.md) — required pattern for stateful dialects
8. [Composed Dialect (All Base Effects)](08_composed_simple.md) — sum composition with `Infallible`
9. [Composed Dialect (Mixed Effects)](09_composed_mixed.md) — composed machine effects via `Lift`
10. [Error Propagation](10_error_propagation.md) — interpreter errors and dialect errors

## Summary Table

| Dialect type | Effect type | Error type | Uses seeds | Notes |
| --- | --- | --- | --- | --- |
| Pure (arith, cmp) | `Infallible` | `Infallible` | No | Only base effects |
| Control flow (cf) | `Infallible` | `Infallible` | No | Uses `Jump` |
| Checked semantics | `Infallible` | Custom | No | Returns `InterpError::Dialect(...)` |
| Calls (function) | `Infallible` | `Infallible` | Yes | Uses `FunctionSeed` as a reusable entrypoint |
| SCF | `Infallible` | `Infallible` | Uses `BlockSeed` | Operation-specific orchestration stays inline |
| Stateful dialect | custom `DE` | `Infallible` or custom | Optional | Uses `Machine(DE)` |
| Composed language | composed `DE` | composed `DErr` | Depends | `Lift` composes sum types |
