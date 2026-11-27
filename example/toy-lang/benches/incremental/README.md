# Analysis-database benchmark

Times a small compile pipeline on toy-lang's `lowered` stage, run through
`kirin-rewrite`'s `Database`. It splits each pass into analysis, rule, and
`run_pass` time and counts the rewrites. Each case runs in two modes:

- `database`: the database keeps a result until a pass edits the program.
- `recompute`: every result is dropped before every pass
  (`Database::program_mut`), as a compiler with no analysis cache would do.

## Run

From the repository root:

```sh
cargo bench -p toy-lang --bench incremental -- \
  --sizes 10 100 300 --repeats 5 --warmups 1 --json /tmp/incremental.json
```

Defaults are both shapes, sizes 10 and 100, 8 units per function, every case,
10 repeats and 2 warmups. `--per-pass` prints each pass of the median sample.
`call_chain` at size 300 takes about a minute per row.

## Workload

`size` functions `@f0` .. `@f{size-1}` and a `@leaf`, each `@fi(%x, %q)`
never reading `%q`. Each function holds `units` three-statement units and
calls one other function:

| Shape | Calls |
| --- | --- |
| `independent` | every `@fi` calls `@leaf` |
| `call_chain` | `@fi` calls `@f{i+1}`; the last calls `@leaf` |

A *dirty* function's units are `%a = constant k; %b = add %a, %a; %acc = add
%acc', %b`, and it passes `%d = mul %x, %x` as its callee's unread `%q`. A
*clean* function has the same number of statements, none foldable
(`%a = sub %x, %x`), and passes `%x`.

| Case | Dirty functions |
| --- | --- |
| `cold` | all |
| `edit_first` | `@f0` only |
| `edit_last` | the last `@fi` only |

The `edit_*` cases are a recompile after editing one function: every other
function is already folded.

## Pipeline

`fold` then `free`, repeated until a round edits nothing. Both are one
`Database::run_pass` running a `RewriteDriver` with one rule from
`toy_lang::analysis_clients`:

- `fold` requires stage-wide constprop (`ConstPropFacts`) and replaces each
  constant `i64` `arith` result with a new `constant`.
- `free` requires stage-wide demand (`DemandFacts`) and moves the uses of each
  undemanded `arith` result onto a new `constant 0`.

The driver's dead-code cleanup erases what the rules leave unused. Both
analyses root every function of the stage, with unknown arguments (constprop)
or a demanded return (demand).

## Columns

| Column | Meaning |
| --- | --- |
| `total` | the sum of every pass's `Database::run_pass` |
| `analysis` | analyses computed for the passes' requirements |
| `affected` | the part of `analysis` spent on roots that reach the edited function: what a database that kept unaffected roots' results would still recompute (`edit_*` only) |
| `driver` | the `RewriteDriver` runs |
| `lookup` | the rules' reads of analysis results, inside `driver` |
| `overhead` | `total - analysis - driver`: stage take/restore and `verify_derived` |
| `runs`, `hits` | analysis runs, and passes whose analysis was cached |
| `roots` | roots analyzed over all runs |
| `lookups` | rule reads of analysis results |
| `applied`, `erased`, `sweeps` | the drivers' `RewriteSummary`, summed |

Times are medians over the repeats, each on freshly parsed IR. Parsing,
building rule sets, and the checks are outside the timer.

## Checks

On a first, untimed run of every row: `@f0`'s concrete result at three
inputs is the same before and after, the program has `units` fewer
statements per dirty function, `fold` applied `units` times and `free` once
per dirty function, and the pipeline reached a round with no edits. A row
that fails prints `FAIL` and the benchmark exits nonzero.

## Instrumentation

`toy_lang::analysis_clients::log` records, per thread, each analysis run with
the time of each root and the time and count of the rules' reads. The
benchmark reads it around each pass.
