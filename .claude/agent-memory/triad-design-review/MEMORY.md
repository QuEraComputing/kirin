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

## kirin-prettyless Architecture (Reviewed 2026-03-01)
- `PrettyPrint` trait: catamorphism over IR, generic over `L: Dialect + PrettyPrint`
- Sub-traits: `PrettyPrintName`, `PrettyPrintType` for format specifier projections
- `ScanResultWidth<L>` pre-pass mutates `Document` for alignment; `PrettyPrintExt` requires both
- `RenderStage`: type-erased per-stage rendering; blanket on `StageInfo<L>`
- **Dead code**: `Config.line_numbers` never read by renderer
- **API gap**: `sprint()` vs `sprint_with_globals()` confusing for users
- **Unused**: `lex()` function in kirin-lexer; duplicated tokenize pattern

## kirin-lexer
- Single-file Logos-based tokenizer, clean and self-contained
- MLIR-style syntax: %ssa, ^block, @symbol, #attr
- Token::Error variant mapped to Err(String) in lex() function
- Has `quote` feature for proc-macro token generation (ToTokens impl)
- `EscapedLBrace`/`EscapedRBrace` only used by format string DSL

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

## Dialect Crate Patterns (Full Review 2026-03-01)
- All dialects parameterized by `T: CompileTimeValue + Default` with `PhantomData<T>` markers
- `#[kirin(pure)]`, `#[kirin(speculatable)]`, `#[kirin(terminator)]`, `#[kirin(constant)]` property annotations
- `#[wraps]` enables dialect composition via enum delegation (coproduct/tagged-union)
- `interpret` feature flag pattern: optional dep on `kirin-interpreter`, `interpret_impl.rs` behind cfg
- SmallVec used correctly in cf/scf interpret impls for branch args
- kirin-constant: ideal minimal teaching example (19 + 21 lines)
- kirin-arith and kirin-bitwise have excellent module docs; kirin-cf, kirin-scf, kirin-constant lack them
- Lexical<T> vs Lifted<T> in kirin-function cleanly models two calling conventions
- ArithValue manual Hash/PartialEq uses to_bits() for floats -- correct NaN handling
- **RED**: Call linear scan O(N) in interpret_impl.rs:97-115 -- needs Pipeline::function_by_name index
- **YELLOW**: Duplicate Return in kirin-cf and kirin-function (document, don't refactor)
- **YELLOW**: E0275 on Region-containing types with `#[wraps]` + `HasParser` (Lambda) -- workaround: inline fields
- **YELLOW**: No comparison ops -- ConditionalBranch unusable with built-in dialects only
- **YELLOW**: Div/Rem panic -- Arith interpreter does unchecked division (non-speculatable signals can-fail)
- **YELLOW**: Stage-resolution boilerplate (8 lines) repeated in kirin-function and kirin-scf -- needs helper
- **YELLOW**: `#[kirin(type = T::default())]` undocumented -- used when result type unknown at parse time
- **GREEN**: `Constant<T, Ty>` two-param API confusing (value-first, type-second)
- **GREEN**: SCF uses Block instead of Region -- correct per MLIR SingleBlock convention
- `Function { body: Region }` pattern is undocumented folklore required for pipeline parsing

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
- **YELLOW**: Duplicate arg binding in 3 places
- Good: InterpreterError variants clear, SummaryInserter API discoverable, Args SmallVec well-sized

## Runtime Module Review (2026-02-27, full triad review)
### Core issue: FrameStack doesn't eliminate real duplication
- `FrameStack` returns `Option`; both interpreters convert to `Result` identically
- Duplicated glue: stack/frame.rs:61-85 === abstract_interp/interp.rs:310-334
- FIX: Add `read_or_err<E>`, `write_or_err<E>`, `write_ssa_or_err<E>`, `push_checked<E>` to FrameStack
- Also `active_stage_or_err<E>` to collapse `active_stage_from_frames`
- `push_frame` with max_depth check also duplicated -- `push_checked` eliminates it

### DedupScheduler issues:
- `push` doesn't dedup but `push_unique` does -- incoherent on same type
- Uses `std::HashSet` instead of `FxHashSet`; Block IDs are dense ints (BitVec ideal)
- FIX: Make push always dedup, drop UniqueScheduler subtrait

### Dead code (~150 lines, zero production consumers):
- `BranchBatch`, `Driver`, `VecDequeScheduler`, `ForkAction::Spawn`
- `WorkExecutor`, `WorkLoopRuntime`, `ForkStrategy` -- only used by MockRuntime in tests
- `RuntimeObserver`/`RuntimeEvent` defined but never emitted by real interpreters

### Design principles confirmed:
- Extract abstractions with 2+ real consumers, not speculatively
- External strategy traits needing `&mut` host access hit borrow-checker walls
- Observer hooks should be type params on interpreter, not standalone hierarchy
- Stack (cursor-walk) vs abstract (worklist-drain) loops are too different to unify
- FrameStack should bridge Option->Result gap to truly eliminate Interpreter impl duplication

## Parser/Printer Architecture
- Two-phase parsing: AST (with spans) -> EmitIR -> IR; `ASTSelf` coinductive wrapper for self-ref types
- `StageDialects` HList: `(L, Tail)` type-level dispatch for multi-dialect pipeline parsing
- Format strings: `#[chumsky(format = "...")]` micro-DSL with `{field}`, `{field:name}`, `{field:type}`
- `ScanResultWidth` pre-pass for result alignment in pretty printer
- Key issues: EmitContext string allocs, two-pass re-parsing, 3 attribute namespaces, AST type leakage
- `generate/ast.rs` ~1000 lines manual Clone/Debug/PartialEq for wrapper enums

## Parser Framework Deep Dive (Full Review 2026-03-01)
- `HasParser` (non-recursive) vs `HasDialectParser` (recursive, GAT-based) -- core duality
- `HasDialectParser::Output<TypeOutput, LanguageOutput>` avoids GAT projection infinite compilation
- `ASTSelf` coinductive wrapper for self-referential parsing -- undocumented fragile trick
- `EmitIR<L>` is a catamorphism; `DirectlyParsable` provides identity morphism
- `ParseStatementText<L, Ctx=()>` + `ParseStatementTextExt` erases unit arg (good ergonomics)
- Format string `{field:option}` drives both parser + printer (bidirectional)
- `FormatVisitor` trait for unified format-driven traversal
- Derive generates ~6 impls: AST, ASTSelf, HasDialectParser, HasParser, EmitIR x2
- Two-pass pipeline parser correctly handles forward refs + mixed-dialect dispatch
- **RED**: Block forward-ref bug: Region::emit registers blocks AFTER body emission (ast.rs:388)
- **RED**: EmitContext panics on undefined SSA/block names (ast.rs:223, ast.rs:278)
- **YELLOW**: EmitContext uses std::HashMap -- should switch to FxHashMap
- **YELLOW**: `tokens.to_vec()` in parse_one_declaration (syntax.rs:136) -- O(n^2) for N declarations
- **YELLOW**: Generated names (FooAST, FooASTSelf) leak into errors -- rename to __Foo...
- **YELLOW**: No systematic roundtrip tests (only one trivial test in function_text/tests.rs)
- **YELLOW**: `for<'src> HasParser<'src, 'src>` HRTB not discoverable -- need ParseDialect helper trait
- **YELLOW**: No Levenshtein suggestions for format string field names (unlike stage names)
- **GREEN**: `BoundsBuilder::new` takes `_ir_path` but ignores it -- dead param
- **GREEN**: `input_requires_ir_type` duplicated for ChumskyLayout/PrettyPrintLayout (should be generic)
- **GREEN**: `DirectlyParsable` coherence restriction undocumented
- **GREEN**: `collect_existing_ssas` scans entire arena per parse_statement call
