# U2: Parser Runtime (kirin-chumsky + kirin-lexer) -- Dialect Author Review

## Workflow Trace

**Goal**: Parse a custom operation containing a `DiGraph` field.

1. Define the struct with `#[derive(HasParser)]` and a `#[chumsky(format = ...)]` attribute:
   ```rust
   #[chumsky(format = "$graph_func {0}")]
   GraphFunc(DiGraph, #[kirin(type = T)] ResultValue),
   ```
   The derive generates a parser that recognizes `digraph ^name(...) { ... }` for the `{0}` position.

2. For projected format (custom syntax around graph parts):
   ```rust
   #[chumsky(format = "fn {:name}{sig} ({graph:ports}) captures ({graph:captures}) {{ {graph:body} }}")]
   FuncBody { graph: DiGraph, sig: Signature<T> }
   ```
   Projections `{graph:ports}`, `{graph:captures}`, `{graph:body}` parse individual components.

3. The `EmitContext` supports relaxed dominance for graph bodies, enabling forward references.

**Friction points**: The projection syntax `{field:projection}` is powerful but undiscoverable -- a dialect author must know the exact projection names (`ports`, `captures`, `body`, `args`, `yields`). No compile-time error if a projection name is misspelled; it would surface at parse time.

## Findings

### [P1] [likely] Finding -- No discoverable documentation for projection names

The set of valid body projections (`ports`, `captures`, `body`, `args`, `yields`, `name`) is implicit in the code generation. A dialect author trying `{graph:edges}` or `{block:statements}` would get a confusing error. The projection vocabulary should be documented in one place, ideally as a doc comment on the format string attribute.

**Files**: `crates/kirin-chumsky/src/parsers/graphs.rs` (component parsers), derive codegen (not reviewed here)

### [P2] [likely] Finding -- Graph parser error messages lack context

If a dialect author writes invalid syntax inside a `digraph { ... }` body, the error reports a span offset relative to the full input but the error message from chumsky is generic ("found Token::Identifier, expected Token::Semicolon"). The `labelled("digraph body statements")` helps, but for deeply nested graphs the error chain could be clearer about which graph body failed.

**File**: `crates/kirin-chumsky/src/parsers/graphs.rs:97`

### [P3] [confirmed] Finding -- `port_list` and `capture_list` are identical parsers

Both `port_list` and `capture_list` in `graphs.rs` parse the same grammar (`block_argument` separated by commas). They differ only in their label. The duplication is harmless but could be collapsed into a single function parameterized by label.

**File**: `crates/kirin-chumsky/src/parsers/graphs.rs:16-48`

## Domain Alignment

| Domain Concept | Parser Mapping | Fit |
|---|---|---|
| Graph body syntax (`digraph ^name(...) { ... }`) | `graphs::digraph()` parser | Natural -- matches MLIR graph-region syntax conventions |
| Forward references in graph bodies | `EmitContext::set_relaxed_dominance(true)` | Natural -- creates `Unresolved(Result(0))` placeholders, resolved at build time |
| Edge-marked statements in ungraphs | `edge` keyword prefix in `ungraph_statement()` | Natural -- clear syntactic marker, matches hyperedge graph literature |
| Projected body format | `{field:ports}`, `{field:body}` projections | Awkward (discoverable) -- powerful but projection vocabulary is implicit |
| Capture clause | `capture(...)` keyword syntax | Natural -- mirrors lambda capture syntax in PLs |

## Strengths

- The three-path `ParseEmit` design (derive, `SimpleParseEmit`, manual) gives dialect authors exactly the right amount of control at each complexity level.
- Relaxed dominance mode (`set_relaxed_dominance`) elegantly handles graph body forward references without changing the core SSA model.
- The projection system (`{field:ports}`, `{field:body}`, etc.) enables fully custom surface syntax while reusing the framework's graph construction machinery.
- The `HasDialectParser` / `HasDialectEmitIR` separation keeps language-independent parsing separate from language-dependent emission -- clean architectural boundary.
