# Triad Design Review Agent Memory

## kirin-ir Core Architecture
- Arena-based IR: `Arena<I: Identifier, T>` backed by `Vec<Item<T>>` with soft-delete pattern
- IDs are newtype wrappers around `Id(usize)` generated via `identifier!` macro
- `GetInfo<L>` trait on ID types dispatches to the correct arena in `StageInfo<L>`
- Intrusive linked lists for statements (in blocks) and blocks (in regions)
- Two symbol scopes: `Symbol` (stage-local) and `GlobalSymbol` (cross-stage, in Pipeline)
- Function hierarchy: `Function` -> `StagedFunction` -> `SpecializedFunction`
- `Dialect` is a god-trait (14 supertraits) -- intentional MLIR-style choice, mitigated by derive macros
- `Successor` is a newtype over `Block` ID with free conversion -- semantically a control flow edge
- `DenseHint` and `SparseHint` provide per-arena-item metadata storage
- `InternTable<T, Key>` for symbol interning (stage-local and global)
- `Signature<T, C>` with `SignatureSemantics` trait for dispatch (ExactSemantics, LatticeSemantics)
- Error-as-recovery pattern: `StagedFunctionError`/`SpecializeError` preserve args for `redefine_*`

## Key Review Findings (Core IR) - 2026-02-24
- `InternTable` uses `std::HashMap` instead of `FxHashMap` -- should switch (free perf win)
- `FxHashSet<Use>` in SSAInfo is allocation-heavy; SmallVec<[Use; 2]> recommended
- `Arena::iter()` has redundant identity `.map()` call
- Arena::gc() returns IdMap but no automated reference remapping exists -- correctness hazard
- Successor/Block have free bidirectional conversion, undermining newtype safety
- `IndexMap<CompileStage, StagedFunction>` in FunctionInfo is heavyweight for 1-3 entries
- `TestSSAValue`/`SSAKind::Test`/`SSAKind::Builder*` leak into public API
- `DenseHint` has overly restrictive `Clone` bound on Index impls
- Three-level function hierarchy has high ceremony for simple use cases
- `GetInfo<L>` pass-the-context pattern is ergonomically heavy -- WithStage wrapper recommended
- Lattice traits well-documented with algebraic laws (improved from earlier)
- StatementIter now has DoubleEndedIterator (fixed from earlier review)
- Builder APIs (BlockBuilder, RegionBuilder, bon builders) are ergonomic
- `CompileTimeValue` blanket impl is maximally permissive (any Clone+Debug+Hash+PartialEq)

## kirin-lexer
- Single-file Logos-based tokenizer, clean and self-contained
- MLIR-style syntax: %ssa, ^block, @symbol, #attr
- Token::Error variant mapped to Err(String) in lex() function
- Has `quote` feature for proc-macro token generation (ToTokens impl)

## Derive Infrastructure Architecture
- Three-layer: `kirin-derive-core` -> `kirin-derive-dialect` -> `kirin-derive` (proc-macro)
- `Layout` trait: type-level extensibility for extra attributes (4 associated types)
- Consumers: `StandardLayout` (default), `ChumskyLayout`, `CallSemanticsLayout`
- `Scan`/`Emit`: two-pass visitor (analysis + synthesis) over IR tree
- `Dialect` derive generates 16 trait impls (10 field iters, 4 properties, builder, marker)
- **Single-parse path**: `derive_statement` parses once, reuses `Input<StandardLayout>` across all 16 generators (fixed from earlier 15x)
- **Stringly-typed field classification** matches on type name strings like "SSAValue"
- Token builder boilerplate in `tokens/trait_impl.rs` (~440 lines, 3 near-identical builders)
- **Duplicated `build_pattern`** across interpretable, call_semantics, and property derives
- `FieldData<L>` / `FieldInfo<L>` have manual Clone impls that could be `#[derive(Clone)]`
- `FieldAccess<'a>` has unused phantom lifetime
- PhantomData fields require `#[kirin(default)]` -- every dialect encounters this
- `error_unknown_attribute` doesn't handle `callable` -- no breadcrumb for discoverability
- Property lattice partially validated: speculatable->pure checked, constant->pure not

## Dialect Crate Patterns
- All dialects parameterized by `T: CompileTimeValue + Default` with `PhantomData<T>` markers
- `#[kirin(pure)]`, `#[kirin(speculatable)]`, `#[kirin(terminator)]`, `#[kirin(constant)]` property annotations
- `#[wraps]` enables dialect composition via enum delegation
- `interpret` feature flag pattern: optional dep on `kirin-interpreter`, `interpret_impl.rs` behind cfg
- **Branch ops lack block arguments** -- highest priority structural gap
- **Duplicate Return** in kirin-cf and kirin-function, potential parser conflict
- **E0275** on Region-containing types with `#[wraps]` + `HasParser` (Lambda)
- **No comparison ops** -- ConditionalBranch unusable without user-defined comparisons
- **Div/Rem panic** -- Arith interpreter does unchecked division
- `kirin-bitwise` and `kirin-scf` lack interpreter impls
- **SCF uses Block instead of Region** -- non-nestable structured control flow
- kirin-arith and kirin-bitwise have excellent module docs; kirin-cf, kirin-scf, kirin-constant, kirin-function lack them
- `Constant<T, Ty>` two-param API is confusing (value-first, type-second)
- kirin-interval: Div/Rem top-approximation is sound but imprecise; enhancement not bug
- `Function { body: Region }` pattern is undocumented folklore required for pipeline parsing
- Lexical<T> vs Lifted<T> in kirin-function cleanly models two calling conventions

## Interpreter Architecture
- `Continuation<V, Ext>` with `Infallible` for abstract, `ConcreteExt` for concrete
- `Fork` in base enum (not Ext) because dialect impls generic over `I: Interpreter`
- `call_handler` fn pointer on AbstractInterpreter: type-erased, freezes `L` at install time (intentional: L=composed type)
- `CallSemantics::Result`: `V` (concrete) vs `AnalysisResult<V>` (abstract)
- `SSACFGRegion` marker provides blanket `CallSemantics` impls for both interpreters
- `Interpretable` trait: single method, clean contract for per-statement dispatch
- `AbstractValue`: documented algebraic contracts for widen/narrow, default narrow=identity
- `WideningStrategy::AllJoins` is misnomer -- widens always, not just at join points
- `SummaryCache`: fixed/computed/tentative entries, linear-scan lookup
- Derive: `#[callable]` shifts CallSemantics default behavior (presence-dependent)
- **RED**: Convergence check (call_semantics.rs:174) only checks return value, not block args
- **YELLOW**: FxHashMap for frame values -- dense Vec recommended
- **YELLOW**: worklist.contains() O(n), needs FxHashSet side-set
- **YELLOW**: Duplicate arg binding in 3 places
- Good: InterpreterError variants clear, SummaryInserter API discoverable, Args SmallVec well-sized

## Parser/Printer Architecture
- Two-phase parsing: AST (with spans) -> EmitIR -> IR; `ASTSelf` coinductive wrapper for self-ref types
- `StageDialects` HList: `(L, Tail)` type-level dispatch for multi-dialect pipeline parsing
- Format strings: `#[chumsky(format = "...")]` micro-DSL with `{field}`, `{field:name}`, `{field:type}`
- `ScanResultWidth` pre-pass for result alignment in pretty printer
- Key issues: EmitContext string allocs, two-pass re-parsing, 3 attribute namespaces, AST type leakage
- `generate/ast.rs` ~1000 lines manual Clone/Debug/PartialEq for wrapper enums

## Parser Framework Deep Dive (Full Review 2026-02-24)
- `HasParser` (non-recursive) vs `HasDialectParser` (recursive, GAT-based) -- core duality
- `HasDialectParser::Output<TypeOutput, LanguageOutput>` avoids GAT projection infinite compilation
- `ASTSelf` coinductive wrapper for self-referential parsing -- undocumented fragile trick
- `EmitIR<L>` is a catamorphism; `DirectlyParsable` provides identity morphism
- `ParseStatementText<L, Ctx=()>` + `ParseStatementTextExt` erases unit arg
- `EmitContext` uses std::HashMap -- should switch to FxHashMap
- `tokens.to_vec()` in parse_one_declaration (syntax.rs:136) -- O(n) alloc per declaration
- Format string `{field:option}` drives both parser + printer (bidirectional)
- `FormatVisitor` trait for unified format-driven traversal
- Derive generates ~6 impls: AST, ASTSelf, HasDialectParser, HasParser, EmitIR x2
- Generated names (FooAST, FooASTSelf) leak into errors -- rename to __Foo...
- `HasDialectParser` should be #[doc(hidden)]
- No systematic roundtrip test (parse-print-parse equality)
- `Config.line_numbers` appears unused in kirin-prettyless
- `ScanResultWidth` pre-pass computes widths; `PipelineDocument` enables cross-stage printing
- Levenshtein suggestions for stage names but not format string field names
