---
name: ir-spec-writing
description: Use when designing new IR constructs, dialect operations, text format syntax, type system extensions, or semantic rules — or when iterating on existing IR specifications. Triggers on requests to design a new IR node type, define dialect text syntax, write a language or IR spec, add new body kinds, or revise operational semantics.
---

# IR Spec Writing

**Announce at start:** State which skill is being used so the user knows what process is driving behavior.

Create text format and semantics specifications for IR constructs through example-driven iterative design. This covers a wide range of IR features:

- **New body kinds** (e.g., graph bodies alongside Block/Region)
- **Dialect operations** (binary ops, control flow, structured control flow, function definitions)
- **New statement patterns** (statements with block bodies, region bodies, successor references, capture lists)
- **Type system extensions** (new type lattices, edge types, compile-time value types)
- **Semantic trait definitions** (purity, terminator behavior, speculatability)

## Core Principle: Text Format First, Data Structures Later

Design the text format and semantics before thinking about Rust types, arenas, or traits. The text format is the user-facing contract — it determines how dialect authors and users think about the IR. Implementation follows from a stable text design.

## Process

### 1. Research Prior Art

Before asking any design questions, search the internet for 3-5 existing systems that solve similar problems. Use subagents in parallel to fetch documentation, specs, and papers. For each reference, extract:

- **Text format / syntax** — how does it look on the page?
- **Semantic model** — what are the core abstractions?
- **What it handles well** and **what it can't represent**

Present a concise summary table of references before proceeding. This grounds the design in real-world tradeoffs rather than abstract reasoning.

Track which references actually influenced design decisions. The final spec's References section should only list references that were used — not everything that was read. Each reference entry should include a URL or citation and a one-line note on what was borrowed from it.

### 2. Identify Use Cases

Collect 3-5 concrete use cases that the design must support. Each should stress a different aspect:

- Different operand patterns (fixed vs variable arity, typed vs untyped)
- Different body patterns (no body, block body, region body, graph body)
- Different composition patterns (standalone vs wrappable via `#[wraps]`)
- Different semantic traits (pure vs effectful, terminator vs non-terminator)
- Edge cases (1000+ nodes, self-references, mixed classical/quantum)

These use cases become the test suite for every design decision.

### 3. Iterative Question-Answer Design

Ask one clarifying question at a time. Each question should:

- Present 2-3 concrete options with short descriptions
- State your recommendation and why
- Be answerable without deep context

After the user answers, immediately trace the decision through at least one use case example. This catches problems early — abstract rules that seem clean often break when applied to real examples.

Common question categories vary by what's being designed:

**For new body kinds:**
- Body structure — what contains what? How does it nest?
- Naming/addressing — sigils, prefixes, arena allocation
- Connection semantics — how are relationships expressed?
- Boundary/interface — how does it connect to enclosing scope?
- Parameter passing — how do external values flow in? When arguments carry mixed semantic roles (e.g., edges vs captured scalars), a single flat list creates ambiguity. Use syntactic separation.
- Multiplicity and cycles — what degenerate cases are allowed?

**For new dialect operations:**
- Operand pattern — how many operands? Fixed or variable? What types?
- Result pattern — how many results? How is the result type determined?
- Body fields — does it contain blocks, regions, or graphs?
- Terminator behavior — does it end a block?
- Semantic traits — pure? speculatable? constant?
- Composition — how does it compose with other dialects via `#[wraps]`?

**For type system extensions:**
- Type lattice — how do types relate to each other?
- Compile-time values — what values can exist at compile time?
- Type inference — how are result types computed from operands?

### 4. Example-Driven Validation

For every design decision, produce a complete example in the proposed syntax. Then check:

- **Are all values defined?** Every `%name` must have a definition site.
- **Does the positional mapping work across split argument groups?** If a construct has multiple argument lists (e.g., `(edge_args...) capture(captured...)`), the enclosing signature maps to the *concatenated* list. Trace the full mapping.
- **Are there naming conflicts?** Check that sigils, keywords, and prefixes don't create parse ambiguity with existing syntax. Kirin uses `%` for SSA values, `^` for block/graph labels, `@` for symbols, `->` for result types.
- **Does it compose?** Can this operation be wrapped in a `#[wraps]` enum alongside existing dialect operations? Do namespace prefixes (e.g., `arith.add`, `cf.br`) work with the new syntax?
- **Does it scale?** What happens with 1000+ instances?
- **Does the example match the stated rules?** Re-read semantic rules after each example.

When an example reveals a problem, fix the design — don't patch the example.

**Pivot signals:** if producing an example requires inventing new addressing or naming infrastructure from scratch, stop and check whether an existing model (SSA def-use, block successors, symbol references) already solves it. Adopting an existing model is almost always better than inventing a new one at spec time.

### 5. Draft the Spec Document

Structure depends on what's being specified:

**For new body kinds or major IR extensions:**
```
# Title
[One-paragraph summary]

## References
[Only references that influenced the design, with URLs and notes]

## Overview
[Core concepts, key syntax, value classification table]

## [Component 1] — grammar, semantics, examples
## [Component 2] — grammar, semantics, examples
## Integration — how it connects to existing constructs

## Semantic Rules
[Consolidated rules with inline code examples]

## Deferred (Backlog)
```

**For new dialect operations:**
```
# Title
[One-paragraph summary]

## References
[Prior art for this operation category]

## Operations
[For each operation: syntax, operands, results, semantic traits, examples]

## Composition
[How these operations compose with other dialects]

## Deferred (Backlog)
```

**Value classification table** (for body kinds): enumerate every way a value can be introduced, its role, and restrictions. This becomes the authoritative reference for scoping and validity rules.

Key writing principles:
- **Examples before rules** — show the syntax, then state the rule
- **One concept per section** — don't mix distinct patterns in the same section
- **Inline code examples in semantic rules** — every rule should have a 3-5 line example
- **Keep examples correct** — run the spec review loop to catch inconsistencies
- **Show existing kirin patterns** — when the new design parallels an existing dialect pattern (e.g., block bodies like SCF `if`/`for`, region bodies like `lambda`, terminators like `yield`/`ret`/`br`), reference the existing pattern explicitly

### 6. Spec Review Loop

After writing the spec, dispatch a code-reviewer subagent to check for:

1. Internal consistency — do examples match stated rules?
2. Completeness — do examples expose gaps in rules?
3. Ambiguity — can the syntax be parsed two ways?
4. Contradictions — do rules conflict?

Fix all high-confidence issues. Re-run the review until clean. Then ask the user to review.

### 7. Iterate

The user will likely have feedback that changes the design. Common patterns:

- **"This doesn't make sense for [use case]"** — trace through the use case, find the broken assumption, fix the rule
- **"This is too verbose"** — look for syntax that can be inferred or defaulted
- **"How does X work with Y?"** — an interaction between two features wasn't specified; add a rule and example
- **"We should use the same convention as [existing feature]"** — consistency with existing IR syntax is more important than local elegance

After each round of feedback, update the spec and re-run the review loop.

## Existing Kirin Patterns to Build On

When designing new constructs, these existing patterns are the vocabulary to extend:

| Pattern | Syntax | Example Dialects |
|---------|--------|-----------------|
| Binary op | `%r = op %a, %b -> Type` | arith, bitwise, cmp |
| Unary op | `%r = op %a -> Type` | arith (neg), bitwise (not) |
| Constant | `%r = constant value -> Type` | constant |
| Block body | `op ... { ^label() { stmts; yield %v; } }` | scf (if, for) |
| Region body | `op ... { ^bb0() { ... } ^bb1() { ... } }` | function (lambda) |
| Terminator | `ret %v` / `yield %v` / `br ^target(args)` | function, scf, cf |
| Successor ref | `^target(%args)` | cf (branch, cond_br) |
| Capture list | `captures(%x, %y)` | function (lambda, bind) |
| Namespace prefix | `arith.add`, `cf.br` | via `#[chumsky(format = "...")]` |

New designs should reuse these patterns where possible rather than inventing parallel conventions.

## Anti-Patterns

- **Designing data structures first** — leads to syntax that serves the implementation rather than the user
- **Abstract rules without examples** — rules that aren't validated against concrete use cases will have bugs
- **Fixing examples instead of fixing rules** — if an example doesn't work with the rules, the rules are wrong
- **Implicit capture / implicit scoping** — always make value provenance explicit in the syntax; implicit resolution creates parser ambiguity and user confusion
- **Naming-based semantics** — don't rely on names to convey semantic meaning; use structural syntax instead
- **Single argument list for mixed-purpose inputs** — when arguments carry two different semantic roles, use syntactic separation to make each argument's role unambiguous from the syntax alone
- **Ignoring composition** — a dialect operation that can't be wrapped in a `#[wraps]` enum or used with namespace prefixes won't compose with other dialects

## Rationalization Table

| Temptation | Rationalization | Reality |
|-----------|----------------|---------|
| Design data structures first | "I need to see the Rust types to understand the design space" | Types constrain the format to what's easy to represent, not what's natural for the domain. Graph body design showed this: arena-first thinking led to `NodeId` everywhere; text-first thinking led to `^label` syntax that users actually understand. |
| Write rules without examples | "The rule is clear from the grammar" | Grammar rules that look unambiguous in isolation collide when composed. The graph body spec had three rules that individually parsed fine but created ambiguity when a node had both edge args and captures. Examples caught it; grammar alone didn't. |
| Fix the example to match the rule | "The example has a typo, the rule is right" | If the example doesn't work with the rule, the rule hasn't been tested against reality. Fixing examples instead of rules is how specs accumulate silent contradictions that surface as parser bugs. |
| Skip the composition check | "This dialect is standalone, composition doesn't matter yet" | Every standalone dialect eventually gets wrapped in a `#[wraps]` enum. Namespace prefix conflicts, parse ambiguity with existing sigils, and format string incompatibilities are cheaper to find during spec than after 4 layers of implementation. |
| Use implicit scoping for brevity | "Explicit capture lists are too verbose" | Implicit resolution creates parser ambiguity (is `%x` a local or captured value?) and user confusion about value provenance. The verbosity of explicit capture pays for itself in unambiguous parsing and readable IR dumps. |

## Spec Location

Save specs to the design directory (see AGENTS.md Project structure) unless the user specifies otherwise. Commit after the review loop passes.
