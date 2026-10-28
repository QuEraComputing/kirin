# Pattern Language

Formal definition of the pattern matching language used in defining Kirin's rewrite rules. 
Currently, operation types and attributes (e.g. constant's value) are not considered. 
This is a planned extension.

## 1. Grammar

Metavariables: 
* $x, y$ value names; 
* $s$ op names (same namespace as values); 
* $O$ ops (referenced as `Dialect::op`, e.g. `Arith::add`); 
* $f$ native functions; 
* $n$ pattern names.

$$
\begin{align*}
p ::=\ & \textsf{pattern}\ @n\ \{\ e\ \} && \text{patterns} \\
e ::=\ & \textsf{let}\ x = \square;\ e && \text{hole} \\
\mid\ & \textsf{let}\ s\ @\ (y_1, \ldots, y_n) = O(x_1, \ldots, x_m);\ e && \text{op match} \\
\mid\ & \textsf{let}\ (y_1, \ldots, y_n) = f(x_1, \ldots, x_m);\ e && \text{native computation} \\
\mid\ & \textsf{constraint}\ x;\ e && \text{constraint} \\
\mid\ & \textsf{rewrite}\ s\ \{\ r\ \} && \text{rewrite} \\
r ::=\ & w \mid a;\ r && \text{rewrite body} \\
w ::=\ & \epsilon \mid \textsf{advance} \mid \textsf{skip} \mid \textsf{interrupt} \mid \textsf{walk}\ x && \text{walk result} \\
a ::=\ & \textsf{let}\ (y_1, \ldots, y_n) = O(x_1, \ldots, x_m) && \text{build} \\
\mid\ & \textsf{let}\ (y_1, \ldots, y_n) = f(x_1, \ldots, x_m) && \text{native computation} \\
\mid\ & \textsf{replace}\ y\ \textsf{with}\ x && \text{redirect uses} \\
\mid\ & \textsf{erase}\ s && \text{erase}
\end{align*}
$$

The grammar is split into two sets of statements: 

1. $e$: the matching statements that define the ops being matched and any constraints upon them.
   Also includes the $\textsf{rewrite}\ s\ \{\ r\ \}$ statement which declares the body of rewrite actions.
2. $r, a$: the rewrite body and actions. These define the actions that modify the IR: 
   creating, replacing, and erasing ops. Note that native computations in a rewrite body may also
   perform these actions.
3. $w$: the walk result ending the rewrite body, telling the rewrite driver how to go on walking
   once the actions have run: go on ($\epsilon$ or $\textsf{advance}$), go on without walking the
   statements nested in the root ($\textsf{skip}$), end the current walk ($\textsf{interrupt}$),
   or as the value $x$ a native computation of the rewrite body returned says
   ($\textsf{walk}\ x$).

#### Clarifications
Op matching binds both the op at $s$ and its results at $y_i$. We bind $s$ to have some kind
of name with which to reach zero-result ops. This allows us to later $\textsf{erase}$ them.
Op building binds only its results: a built op is never the root, replaced, or erased, so a name
for it could never be used.

$\textsf{constraint}\ x$ takes a boolean value, which in the current grammar, can only be produced by a
native computation. The typing rules below do not enforce this due to the current absence of a type system,
but the kirin-wildcard rewrite rule macro does.

## 2. Operation and Function Signatures

An op signature is defined as an argument count and a result count, $O : m \to n$; its arguments are its
[`kirin_ir::SSAValue`](../crates/kirin-ir/src/node/ssa.rs#L10-L17) fields and its results are its
[`kirin_ir::ResultValue`](../crates/kirin-ir/src/node/ssa.rs#L19-L22) fields, each in declaration
order. 

A native function signature is defined by its argument count and result types:
$f : m \to (\tau_1, \ldots, \tau_n)$, $\tau_i \in \{\textsf{ir}, \textsf{host}\}$. 
The result types currently just describe whether it is an IR value the function builds or a host value it computes. 

$\textsf{let}$ statements bind their signatures: $m$ arguments, $n$ bound names (`_` counts as a name).

## 3. Sorts, contexts, judgments

$$\tau ::= \textsf{ir} \mid \textsf{host} \qquad \sigma ::= \tau \mid \dagger \qquad \Gamma ::= \cdot \mid \Gamma, x{:}\sigma \qquad \Sigma ::= \cdot \mid \Sigma, s \mapsto (y_1, \ldots, y_n)$$

- $\textsf{ir}$: an IR SSA value. 
- $\textsf{host}$: a value a native function computes; never IR.
- $\dagger$: a replaced op result; its uses have been redirected and it must therefore not be referenced again.
- $\sigma$ is the set of sorts $\tau$ plus $\dagger$ needed to tombstone replaced results.
- $\Gamma$ maps each value name to its sort.
- $\Sigma$ maps each matched op to its results' names. It records two things $\Gamma$ cannot:
  - Ownership. Only matched ops are in $\Sigma$, so only they can be the root, have their results
    replaced, or be erased. Holes and the results of built ops ($\text{B\footnotesize UILD}$) are instead
    $\textsf{ir}$ in $\Gamma$ just like matched results.
  - Which results belong to which op. $\Gamma$ holds only values, with no record of the op that
    produced them, so $\text{R\footnotesize EPLACE}$ needs $\Sigma(s)$ to check that $y$ is a result of a
    matched op, and $\text{E\footnotesize RASE}$ needs it to know which names leave scope with $s$.

Given the current absence of a proper type system, we define only well-formedness judgments over the grammar. 
These are split for the match statements and rewrite statements: 
- $\Gamma;\Sigma \vdash_{\textsf{m}} e\ \textsf{ok}$: match statements (§4);
- $\Gamma;\Sigma \vdash_{\textsf{rw}} r\ \textsf{ok}$: rewrite actions (§5);
- $\vdash p\ \textsf{ok}$: patterns (§6).

The split carries meaning the syntax does not. A native computation has the same syntax in $e$ and
$r$, so it must be the judgment checking it that decides whether it may produce $\textsf{ir}$ (only
under $\vdash_{\textsf{rw}}$). Likewise, the judgment decides whether an op is matched
($\text{M\footnotesize ATCH}$, added to $\Sigma$) or built ($\text{B\footnotesize UILD}$): the two
differ only in that a matched op is named with $s\ @$, and the surface syntax writes both as `op![..]`.

## 4. Match rules

$$\frac{\Gamma, x{:}\textsf{ir};\ \Sigma \vdash_{\textsf{m}} e\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{m}} \textsf{let}\ x = \square;\ e\ \textsf{ok}}\ \text{H\footnotesize OLE}$$

$$\frac{O : m \to n \qquad \Gamma(x_j) = \textsf{ir} \qquad \Gamma, y_1{:}\textsf{ir}, \ldots, y_n{:}\textsf{ir};\ \Sigma, s \mapsto (y_1, \ldots, y_n) \vdash_{\textsf{m}} e\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{m}} \textsf{let}\ s\ @\ (y_1, \ldots, y_n) = O(x_1, \ldots, x_m);\ e\ \textsf{ok}}\ \text{M\footnotesize ATCH}$$

$$\frac{f : m \to (\textsf{host}, \ldots, \textsf{host}) \qquad \Gamma(x_j) \in \{\textsf{ir}, \textsf{host}\} \qquad \Gamma, y_1{:}\textsf{host}, \ldots, y_n{:}\textsf{host};\ \Sigma \vdash_{\textsf{m}} e\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{m}} \textsf{let}\ (y_1, \ldots, y_n) = f(x_1, \ldots, x_m);\ e\ \textsf{ok}}\ \text{N\footnotesize ATIVE}$$

$$\frac{\Gamma(x) = \textsf{host} \qquad \Gamma;\ \Sigma \vdash_{\textsf{m}} e\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{m}} \textsf{constraint}\ x;\ e\ \textsf{ok}}\ \text{C\footnotesize ONSTRAINT} 
\qquad \frac{s \in \mathrm{dom}(\Sigma) \qquad \Gamma;\ \Sigma \vdash_{\textsf{rw}} r\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{m}} \textsf{rewrite}\ s\ \{\ r\ \}\ \textsf{ok}}\ \text{R\footnotesize EWRITE}$$

## 5. Rewrite rules

$$\frac{w \in \{\epsilon, \textsf{advance}, \textsf{skip}, \textsf{interrupt}\}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} w\ \textsf{ok}}\ \text{D\footnotesize ONE}
\qquad \frac{\Gamma(x) = \textsf{host}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} \textsf{walk}\ x\ \textsf{ok}}\ \text{W\footnotesize ALK}$$

$$\frac{O : m \to n \qquad \Gamma(x_j) = \textsf{ir} \qquad \Gamma, y_1{:}\textsf{ir}, \ldots, y_n{:}\textsf{ir};\ \Sigma \vdash_{\textsf{rw}} r\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} \textsf{let}\ (y_1, \ldots, y_n) = O(x_1, \ldots, x_m);\ r\ \textsf{ok}}\ \text{B\footnotesize UILD}$$

$$\frac{f : m \to (\tau_1, \ldots, \tau_n) \qquad \Gamma(x_j) \in \{\textsf{ir}, \textsf{host}\} \qquad \Gamma, y_1{:}\tau_1, \ldots, y_n{:}\tau_n;\ \Sigma \vdash_{\textsf{rw}} r\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} \textsf{let}\ (y_1, \ldots, y_n) = f(x_1, \ldots, x_m);\ r\ \textsf{ok}}\ \text{C\footnotesize OMPUTE}$$

$$\frac{s \in \mathrm{dom}(\Sigma) \qquad y \in \Sigma(s)  \qquad \Gamma(y) = \textsf{ir} \qquad \Gamma(x) = \textsf{ir} \qquad x \neq y \qquad \Gamma[y \mapsto \dagger];\ \Sigma \vdash_{\textsf{rw}} r\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} \textsf{replace}\ y\ \textsf{with}\ x;\ r\ \textsf{ok}}\ \text{R\footnotesize EPLACE}$$

$$\frac{\Sigma(s) = (y_1, \ldots, y_n) \qquad \Gamma \setminus \{y_1, \ldots, y_n\};\ \Sigma \setminus s \vdash_{\textsf{rw}} r\ \textsf{ok}}{\Gamma;\ \Sigma \vdash_{\textsf{rw}} \textsf{erase}\ s;\ r\ \textsf{ok}}\ \text{E\footnotesize RASE}$$

- All bound names, both results and ops, are assumed to be fresh and distinct.
- $\text{B\footnotesize UILD}$ adds nothing to $\Sigma$: a built op is never the root, replaced, or erased.
- Native results in the match must be $\textsf{host}$: the match may still fail, so a match-time native
  cannot build IR. In the rewrite body each result has the sort its signature gives; a $\textsf{host}$
  result can only flow into other native calls (and, once attributes exist, attribute positions).
- $\text{R\footnotesize EPLACE}$ kills its target: no later action uses it, replaces it again, or uses it as a replacement.
- $\text{E\footnotesize RASE}$ After erasing an op $s$, it and its
  results are out of scope. Erasing $s$ while one of its results is still used is a mistake in the
  rule, reported when it runs (§7).
- $\text{W\footnotesize ALK}$: $x$ must be computed by a native computation of the rewrite body,
  not of the match. $\Gamma$ does not record where a $\textsf{host}$ value was computed, so the
  rule does not show this; the macro checks it.

## 6. Patterns

$$\frac{\cdot;\ \cdot \vdash_{\textsf{m}} e\ \textsf{ok} }{\vdash \textsf{pattern}\ @n\ \{\ e\ \}\ \textsf{ok}}$$

Additionally, a pattern must also satisfy the following conditions.
- Every matched op feeds the root, the op named by $\textsf{rewrite}$. A traversal starting
  at the root and following each op's arguments back to the matched ops that produce them, must
  reach every matched op. Every hole must also be an argument of one of the ops reached. A pattern
  therefore cannot match an op or bind a hole that has no bearing on the root.
- The rewrite body contains at least one `replace`, `erase` or native computation before its
  walk result. Otherwise the rule would only build values that nothing uses, or only steer the walk
  without changing the IR. A native computation counts because it
  may act on the IR itself; what it erases or replaces is invisible to the macro, so the checks
  on what the body uses after removing it (§5) do not cover a native's actions.

## 7. Surface syntax (`#[rewrite_rule]`)

The recommended way of constructing a Rewrite Pattern in kirin is via the [`#[rewrite_rule]`](../crates/kirin-derive-wildcard/src/lib.rs#L10-L20) macro. 
It wraps around a Rust function but only allows statements in the pattern language grammar in its body.
The table below describes the conversion from the the grammar syntax above to the surface Rust syntax:


| Core | Rust |
|---|---|
| $\textsf{let}\ x = \square$ | Any name used as a value before any `let` binds it (declared at first use). Also anonymous `_` as an op argument |
| $\textsf{let}\ s\ @\ (\vec y) = O(\vec x)$ | `let s @ (y1, .., yn) = op![Dialect::op(x1, .., xm)];`. Binding for 1-result: `s @ y`, and 0-results: `s @ ()` |
| "" | Without binding the op to `s`: `let (y1, .., yn) = op![..];` or `let y = op![..];` |
| $\textsf{let}\ (\vec y) = f(\vec x)$ | `let (y1, .., yn) = f(x1, ..);` or `let y = f(..);` |
| $\textsf{let}\ y = f(\vec x);\ \textsf{constraint}\ y$ | `f(x1, ..);`, with `f` returning `bool`|
| $\textsf{rewrite}\ s\ \{\ r\ \}$ | `rewrite!(s, { .. })` or `rewrite!(y, { .. })` with $y \in \Sigma(s)$
| $\textsf{erase}\ s$ | `erase!(s)` or `erase!(y)` with $y \in \Sigma(s)$ |
| $\textsf{replace}\ y\ \textsf{with}\ x$ | final `replace!(y, x)`, $y \in \Sigma(s)$  |
| $\textsf{rewrite}\ s\ \{\ \textsf{replace}\ y\ \textsf{with}\ x;\ \}$ | final `replace!(y, x)`, $y \in \Sigma(s)$ without having to write a `rewrite!()` block|
| $\textsf{rewrite}\ s\ \{\ \textsf{erase}\ s;\ \}$ | final `erase!(s)` without having to write a `rewrite!` block|
| $\textsf{advance}$, $\textsf{skip}$, $\textsf{interrupt}$ | `advance!()`, `skip!()`, `interrupt!()` as the last expression of a `rewrite!` body, without a semicolon |
| $\textsf{let}\ y = f(\vec x);\ \textsf{walk}\ y$ | `f(x1, ..)` as the last expression of a `rewrite!` body, without a semicolon |
| $\textsf{walk}\ y$ | `y`, bound by a native computation of the `rewrite!` body, as its last expression, without a semicolon |

A `_` result cannot be replaced; an op with one can still be erased if the rule checks it is unused.

The macro defines a `language` parameter describing the kirin Dialect that the rule aims to match.
The function may be generic, with `language = L` naming one of its generic parameters.
This allows the rule to apply to any language that implement the `where` bounds the rule 
describes. Normally, these are the dialects the rule should apply to  e.g. `L: From<Arith<L::Type>>`.
See [dialect-rewrite-rules.md](design/dialect-rewrite-rules.md).

## 8. Where each condition is enforced

| Condition | Checked by |
|---|---|
| fresh names, no use before `let` | macro |
| built ops in a rewrite block are unnamed (no `s @` in a `rewrite!` body) | macro |
| op arguments are all $\textsf{ir}$ (no match-time native results) | macro |
| root, `replace` and `erase` targets are matched in `op!` statements| macro |
| $\dagger$ and removal after `erase`: nothing used afterwards | macro
| erasing an op whose results are still used | not checked statically: kirin-ir's [`Rewriter::erase_statement`](../crates/kirin-ir/src/rewrite/rewriter.rs#L451-L458) rejects it ([`StatementResultsInUse`](../crates/kirin-ir/src/rewrite/rewriter.rs#L109-L111)) and the pass quarantines the stage |
| $\textsf{rooted}$, $\textsf{acts}$ | macro ($\textsf{rooted}$ in its verification pass) |
| a walk result is last, at most once, and $\textsf{walk}\ x$ reads a native result of the rewrite body | macro; for patterns built without it, [`RuleSet::add`](../crates/kirin-rewrite/src/rule/rule_set.rs) |
| $O : m \to n$ (argument and result counts), $O$ in the language | rustc, through the generated constructor call `new_<op_name>`; for a generic rule, through the bounds its author writes |
| native call arity and result sorts $\tau_i$ | not yet checked |

## 9. Future Work

- A pattern registry into which the derived macro functions get added to.
- Type matching (type variables, op type schemes)
- Attributes (compile-time value fields, e.g. [`Constant::value`](../crates/kirin-constant/src/lib.rs#L14)) are not matchable yet.
- Injectivity: may two op names match the same statement?
- `Vec` / `Option` result fields (`scf.if`, calls, `tuple.unpack`): how a rule binds them.
- Implement native function registry to disambiguate between multiple functions with the same name.
  Also to check argument counts and types at compile time.
