# Rewrite benchmark

Runs the same IR edits in Kirin-Rust (`kirin-rewrite`'s `RewriteDriver`, or
raw `Rewriter` edits, inside `Program::run_pass`) and in Kirin-Python
(`kirin.rewrite` rules and `Statement`/`SSAValue` methods). It checks each
result and times both implementations. Kirin-Rust has no full passes yet
(constant folding, inlining, specialization), so the cases cover the rule
driver, dead-code cleanup and single edits.

## Run

From the Rust repository root, with a clean Python checkout that has a `.venv`
(`uv sync --no-dev`):

```sh
python3 benchmarks/rewrite/compare.py \
  --python-checkout /absolute/path/to/python-kirin \
  --output /absolute/path/to/results.json
```

Defaults are every task, sizes 1000 and 4000, 10 repeats and 2 warmups.
`--tasks` selects tasks, `--sizes 100 --repeats 2 --warmups 0` is a quick run.
Sizes must be multiples of 4, as for the shared workloads. Python's
`dce` on `dead_chain` is quadratic (about 3 s per sample at 4000), and it
needs more than its 10,000-sweep budget above size 9999.

## Cases

Every case edits one `@benchmark(x, y, outer, inner)` function.

| Task | Shape | Edit |
| --- | --- | --- |
| `peephole` | `neg_chain`: `size` nested negations of `x` | rewrite `-(-x)` to `x` until nothing changes |
| `no_match` | `chain`, `diamond`, `nested_cfg` from [`../common`](../common) | the same rule, which matches nothing |
| `dce` | `dead_independent`: `size` unused `x + y` | dead-code elimination alone |
| `dce` | `dead_chain`: `x + y + ... + y`, unused | dead-code elimination alone |
| `cascade` | `unroll_3`, `unroll_8` | start with `size / 2` copies of `v - k`, each with its own constant; insert `(v - (k - 1)) - 1` before the matched subtraction until only subtractions of one remain |
| `backward_cascade` | `folds_F_match_M` | keep a prefix of `size` additions and peel `F` double negations from the return, while a second match looks up to `M` addition producers back |
| `bypass` | `chain` | per addition: redirect its uses to its left operand, erase it |
| `insert` | `chain` | per addition: create an unused `constant 0` before it |
| `move` | `chain` | per addition, in order: move it before the terminator |
| `erase` | `dead_independent` | per statement: erase it |

In `backward_cascade`, `F` (folds) is 1, 2, 3, 4 or 8 and `M` (match cost, in
producer lookups per addition) is 0, 8 or 32.
Only the negation pair feeding the return is eligible. Folding it exposes the preceding
pair behind the pre-order cursor. The other match walks the left operand chain of
each addition looking for `(a - b) + b`. The generated prefix contains only
additions and returns `x + size * y` after the tail is stripped. A match cost of
zero keeps rule dispatch while disabling the producer search. For example,
`--tasks backward_cascade --shapes folds_2_match_8` selects two folds with
up to eight lookups per addition. In contrast, `cascade` inserts new producers
throughout the function. At size 4000, `unroll_8` requires 14,000 applications,
which can exceed a worklist driver's per-sweep budget. Rust execution failures
are recorded in the report and cause a nonzero exit after the other cases run.

The rule-driven tasks run to a fixpoint in both implementations:

- Rust: `RewriteDriver::run::<PreOrder>`, which sweeps the rules and then
  erases dead code after each sweep. Two variants are timed: `rust_pattern`,
  a `#[rewrite_rule]` pattern, and `rust_native`, the same match and edit as a
  `StatementRule`. `dce` runs the driver with no rules (`rust_cleanup`).
- Python: `Fixpoint(Chain(Walk(DoubleNeg()), Walk(DeadCodeElimination())))`,
  and `Fixpoint(Walk(DeadCodeElimination()))` for `dce`, as `passes/fold.py`
  does. `max_iter` is 10,000, Rust's `DEFAULT_MAX_SWEEPS`.

Both rules only redirect uses. A match whose result is already unused is not a
change, so each run ends with a sweep that changes nothing.

Python's `Walk` visits a block's statements first to last, so its DCE erases
one link of a dead chain per sweep (`size + 1` sweeps). Rust's cleanup walks
each block from its end and erases the whole chain in one sweep. `sweeps` in
the report shows this.

## Checks

For every variant, the report records the statement count of `@benchmark`
before and after (terminators included), the result of the concrete
interpreter at each of the shared samples, and the run's summary (sweeps,
applications, erasures, or edits). A variant fails if its count before
differs from Python's, its count after or a value differs from the expected
one, or its fixpoint did not converge. Expected values come from the shared
closed-form oracle for edits that keep the arithmetic (`no_match`, `insert`,
`move`) and are `x` for `peephole`, `dce`, `bypass` and `erase`. The unroll oracle
expects `x - k * (size / 2)` and `k * size + 1` statements. The backward
cascade checks its prefix and tail lengths, `size + 1` final statements, and
exactly `F` Rust rule applications and `2 * F` cleanup erasures.

## Timed boundary

- Rust `pass_seconds`: the edit inside the `run_pass` closure, driver
  construction included. `run_pass_seconds`: the whole `run_pass`, adding the
  stage take/restore and the post-pass `verify_derived`, which Python has no
  counterpart for.
- Python `pass_seconds`: the edit on the built `Method`, rule construction
  included. Garbage collection runs between samples and is disabled inside
  the timer.

Building the rule set (Rust), building or parsing the IR, checking, and
dropping the IR are outside the timer. Every sample edits freshly built IR.

The report's ratios divide Python's median by each Rust median.
