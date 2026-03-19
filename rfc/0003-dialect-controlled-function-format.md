# RFC 0003: Dialect-Controlled Specialized Function Text Format

- **Status**: Draft
- **Authors**: roger, claude
- **Created**: 2026-03-19

## Summary

Give dialect authors control over the text format of specialized function declarations. Instead of the hard-coded `stage @A fn @foo(T) -> T; specialize @A fn @foo(T) -> T { body }` syntax, the framework only provides the `specialize @stage` prefix. Everything else — function name, parameter names, types, body layout — is arranged by the dialect's format string using `{function:...}` and `{body:...}` projections. The dialect statement implements a `HasSignature` trait so the framework can extract the function signature from the parsed definition.

### Relationship to MLIR

In MLIR, function-like operations (`func.func`, `gpu.func`, `llvm.func`) fully control their text format. Each dialect implements its own custom parser/printer — there is no framework-imposed function syntax. `FunctionOpInterface` is a semantic interface (argument types, result types, callable), not a syntactic constraint.

Kirin follows the same principle with one addition: the `specialize @stage` prefix. This exists because Kirin has an explicit multi-stage pipeline concept (stage → staged function → specialization) that MLIR does not. The prefix associates the function with a stage — everything after is dialect-controlled.

**Division of responsibility:**

| Concern | Who handles it |
|---------|---------------|
| Call resolution, dispatch | Framework |
| Function name registration (global symbol) | Framework |
| Stage association | Framework (`specialize @stage` prefix) |
| Specialization indexing | Framework |
| Signature extraction for dispatch | Dialect (`HasSignature` trait) |
| Entire text representation of function body | Dialect (format string + projections) |

For truly custom formats beyond what projections support, dialect authors can implement a manual `HasParser`/`PrettyPrint` (analogous to MLIR's `hasCustomAssemblyFormat`).

## Motivation

### Current format is rigid and duplicative

```
stage @circuit fn @bell_pair(Qubit, Qubit) -> Qubit;
specialize @circuit fn @bell_pair(Qubit, Qubit) -> Qubit digraph ^dg0(%q0: Qubit, %q1: Qubit) {
  %q0_h = h %q0 -> Qubit;
  %q0_out, %q1_out = cnot %q0_h, %q1 -> Qubit, Qubit;
  yield %q0_out, %q1_out;
}
```

Problems:
1. **Duplication**: Type information appears three times — `stage` declaration, `specialize` header, and `digraph` port list
2. **No dialect control**: The `fn @name(types) -> type` syntax is hard-coded in `syntax.rs`. Dialect authors cannot change it.
3. **Graph headers are noisy**: The `digraph ^dg0(...)` header is always visible even when redundant with the function signature
4. **Separate `stage` declaration**: Forces a header-only `stage` line before every `specialize`, even for single-specialization functions

### What dialect authors want

Different domains want different text representations:

**Quantum circuits** (graph-based, ports are the interface):
```
specialize @circuit fn @bell_pair(%q0: Qubit, %q1: Qubit) -> Qubit, Qubit {
  %q0_h = h %q0 -> Qubit;
  %q0_out, %q1_out = cnot %q0_h, %q1 -> Qubit, Qubit;
  yield %q0_out, %q1_out;
}
```

**Traditional SSA CFG** (block-based, arguments on entry block):
```
specialize @source fn @factorial(%n: i64) -> i64 {
  ^entry(%n: i64):
    %one = constant 1 -> i64;
    ...
}
```

**Signal processing** (graph-based, ports + captures):
```
specialize @dsp fn @filter(%in: Signal) captures (%coeffs: Vec<f64>) -> Signal {
  digraph {
    ...
  }
}
```

The common pattern: `specialize @stage` prefix is framework-controlled, everything after is dialect-controlled.

## Design

### Format string projections

The dialect's function-body struct uses `{function:...}` and `{body:...}` projections in its format string:

#### `{function:...}` — SpecializedFunction metadata

| Projection | Parses/prints | Source |
|-----------|---------------|--------|
| `{function:name}` | `@symbol_name` | Function's global symbol |

#### `{body:...}` — Graph/Region body structural parts

| Projection | Parses/prints | Source |
|-----------|---------------|--------|
| `{body:ports}` | `%name: Type, %name: Type` | Graph port declarations (DiGraph/UnGraph) |
| `{body:captures}` | `%name: Type, %name: Type` | Graph capture declarations |
| `{body:yields}` | `Type, Type` | Yield types (from yield statement or signature) |
| `{body:args}` | `%name: Type, %name: Type` | Block arguments (for Region/Block bodies) |
| `{body:body}` | The inner statements | Statement list (no graph header, no braces) |
| `{body}` | Full body with header | Current behavior — entire `digraph ^name(...) { ... }` |

`{body:body}` prints only the statements inside the graph/region (no header, no braces). `{body}` prints the full body including header and braces (backward compatible).

### Example: CircuitFunction

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = QubitType)]
#[chumsky(format = "fn {function:name}({body:ports}) -> {body:yields} { {body:body} }")]
pub struct CircuitFunction {
    pub body: DiGraph,
}
```

Text format:
```
specialize @circuit fn @bell_pair(%q0: Qubit, %q1: Qubit) -> Qubit, Qubit {
  %q0_h = h %q0 -> Qubit;
  %q0_out, %q1_out = cnot %q0_h, %q1 -> Qubit, Qubit;
  yield %q0_out, %q1_out;
}
```

The `specialize @circuit` prefix is added by the framework. Everything else comes from the format string.

### Example: FunctionBody (SSA CFG, current style)

```rust
#[derive(Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = T)]
#[chumsky(format = "fn {function:name}({function:params}) -> {function:ret} { {body} }")]
pub struct FunctionBody<T: CompileTimeValue> {
    pub body: Region,
}
```

Text format (unchanged from current):
```
specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64):
    %one = constant 1 -> i64;
    ...
}
```

Here `{function:params}` prints type-only parameter list from the signature, and `{body}` prints the full Region (with block headers). This preserves backward compatibility.

### Example: Signal processing with captures

```rust
#[chumsky(format = "fn {function:name}({body:ports}) captures ({body:captures}) -> {body:yields} { digraph {body:body} }")]
pub struct DSPFunction {
    pub body: DiGraph,
}
```

Text format:
```
specialize @dsp fn @filter(%in: Signal) captures (%coeffs: Vec<f64>) -> Signal {
  digraph {
    ...
  }
}
```

### HasSignature trait

The framework needs to extract a `Signature<L::Type>` from the parsed function statement to construct the `SpecializedFunction`. Today this comes from the hard-coded `fn @name(types) -> type` header. With dialect-controlled format, the dialect provides it:

```rust
/// Extract the function signature from a parsed function-body statement.
///
/// Implemented by dialect types that serve as function bodies (e.g., CircuitFunction,
/// FunctionBody). The framework calls this after parsing to construct the SpecializedFunction.
pub trait HasSignature<L: Dialect> {
    fn signature(&self, stage: &StageInfo<L>) -> Signature<L::Type>;
}
```

For `CircuitFunction`, the signature is derived from the DiGraph's ports (params) and yields (return):
```rust
impl HasSignature<Circuit> for CircuitFunction {
    fn signature(&self, stage: &StageInfo<Circuit>) -> Signature<QubitType> {
        let info = self.body.expect_info(stage);
        let params: Vec<QubitType> = info.edge_ports().map(|p| p.expect_info(stage).ty().clone()).collect();
        let ret = QubitType::Qubit; // or derive from yields
        Signature::new(params, ret, ())
    }
}
```

For `FunctionBody`, the signature comes from the parsed `{function:params}` and `{function:ret}`:
```rust
impl<T: CompileTimeValue> HasSignature<L> for FunctionBody<T> {
    fn signature(&self, _stage: &StageInfo<L>) -> Signature<T> {
        // Signature was already parsed from {function:params} and {function:ret}
        // and stored during emit
        self.cached_signature.clone()
    }
}
```

### Eliminating separate `stage` declarations

With `HasSignature`, the `stage @A fn @foo(T) -> T;` header declaration becomes optional. The framework can infer the staged function's signature from the first specialization's `HasSignature` output.

For single-specialization functions (the common case), the text format simplifies from:
```
stage @circuit fn @bell_pair(Qubit, Qubit) -> Qubit;
specialize @circuit fn @bell_pair(Qubit, Qubit) -> Qubit { ... }
```
to:
```
specialize @circuit fn @bell_pair(%q0: Qubit, %q1: Qubit) -> Qubit, Qubit { ... }
```

The `stage` declaration is only needed for:
- Forward references (function called before it's defined)
- Multiple specializations with different signatures (the `stage` provides the generic signature)

### Parser changes

#### Two-pass architecture preserved

The two-pass architecture stays — it's needed for forward references. But the passes change:

**Pass 1**: Scan for `stage` (optional) and `specialize` keywords. For `stage`, parse the header as today (provides forward-reference signature). For `specialize`, record body offset for pass 2.

**Pass 2**: For each `specialize`:
1. Parse `specialize @stage_name` prefix (framework-controlled)
2. Delegate remaining text to `L::parse_and_emit()` (dialect-controlled)
3. The dialect parser uses `{function:name}`, `{body:ports}`, etc. to parse all components
4. Call `HasSignature` on the emitted statement to get the signature
5. Construct `SpecializedFunction` with the extracted signature and body statement

The key change: the framework no longer parses `fn @name(types) -> type` — the dialect does, via its format string.

### Printer changes

The statement-level printer (`ir_render.rs`) currently doesn't print function headers — that's done by the function text printer. With this RFC:

1. The function text printer emits `specialize @stage_name ` prefix
2. Delegates to the dialect's `PrettyPrint` for everything else
3. The dialect's format-based PrettyPrint handles `{function:name}`, `{body:ports}`, etc.

## Alternatives

### Alternative A: Keep hard-coded format, add options

Add attributes to hide/show components:
```rust
#[kirin(function_format(hide_graph_header, show_ports_in_signature))]
```

**Rejected**: Combinatorial explosion of options. Doesn't give dialect authors real control — just toggles on a fixed format.

### Alternative B: Separate format string for function header

```rust
#[kirin(header_format = "fn {name}({ports}) -> {yields}")]
#[chumsky(format = "{body}")]
```

**Rejected**: Splits the format across two attributes, making it harder to read and reason about the complete text format.

### Recommendation

The unified format string approach (main design) is best:
- One format string describes the complete text representation
- Projections (`{function:...}`, `{body:...}`) reference structural parts clearly
- `HasSignature` cleanly separates IR construction from text format
- Backward compatible via `{body}` (full body with header) and `{function:params}`/`{function:ret}` (signature from declaration)

## Crate impact matrix

| Crate | Impact | Changes |
|-------|--------|---------|
| `kirin-chumsky` | **Primary** | Function text parser: dialect-controlled format dispatch |
| `kirin-chumsky` | **Primary** | New `{function:...}` and `{body:...}` projection parsers |
| `kirin-ir` | **Primary** | `HasSignature` trait definition |
| `kirin-derive-chumsky` | **Primary** | Codegen for `{function:...}` and `{body:...}` projections |
| `kirin-prettyless` | **Secondary** | Function text printer delegates to dialect PrettyPrint |
| `kirin-function` | **Migration** | `FunctionBody` implements `HasSignature`, format string updated |
| `example/toy-qc` | **Migration** | `CircuitFunction`/`ZXFunction` format strings updated |
| `example/toy-lang` | **Migration** | Language format strings updated |

## Migration path

### Phase 1: Add projection support (non-breaking)
- Add `{function:...}` and `{body:...}` projections to format parser
- Add `HasSignature` trait with default impl (returns current hard-coded signature)
- Existing format strings continue to work unchanged

### Phase 2: Migrate function body types
- Update `CircuitFunction`, `ZXFunction`, `FunctionBody` format strings
- Implement `HasSignature` per type
- Make `stage` declarations optional when signature can be inferred

### Phase 3: Simplify function text parser
- Remove hard-coded `fn @name(types) -> type` parsing for `specialize` (keep for `stage`)
- The `specialize` parser delegates entirely to dialect after `specialize @stage`

## Open Questions

1. **`{body:yields}` for Region-based bodies**: Regions don't have explicit yields. Should `{body:yields}` be a compile error for Region fields, or return the function's return type?

2. **Multiple body fields**: Can a dialect struct have multiple graph fields? E.g., `input_graph: DiGraph, output_graph: DiGraph`. If so, which one do `{body:...}` projections reference? Probably disallow — one body field per function statement.

3. **`stage` declaration format**: Should `stage` declarations also be dialect-controlled? Or keep them framework-controlled (since they're just signatures, no body)?

4. **`HasSignature` timing**: The trait is called after parsing. But the builder needs the signature to construct `StagedFunction` in pass 1. For forward references, the `stage` declaration still provides the signature. `HasSignature` is only for pass 2 (specialization). Is this sufficient?

5. **Capture semantics**: `{body:captures}` references graph captures. For Region bodies, there are no captures. Should `{body:captures}` be empty/forbidden for Region, or should it reference something else (e.g., closure-captured values)?

## Reference Implementation Plan

1. Add `{function:...}` and `{body:...}` as new `FormatOption` variants in format parser
2. Add `HasSignature<L>` trait to `kirin-ir`
3. Codegen for projection parsers (port list, capture list, yield types, body-only)
4. Codegen for projection printers
5. Wire `HasSignature` into `second_pass_concrete()` for signature extraction
6. Make `stage` declarations optional when `HasSignature` is available
7. Migrate `CircuitFunction` and `ZXFunction` in toy-qc
8. Migrate `FunctionBody` in kirin-function
9. Update toy-lang
