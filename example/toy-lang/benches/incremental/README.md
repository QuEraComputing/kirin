# Analysis-database benchmark

Times a small compile pipeline on toy-lang's `lowered` stage. It splits each
pass into analysis, rule, and `run_pass` time and counts the rewrites. Each
case runs in two modes:

- `database`: passes run through `kirin-rewrite`'s `Database::run_pass`,
  which computes a pass's required analyses unless a result from the current
  revision is cached. Any edit drops every result.
- `direct`: no database. Each pass computes its analysis itself
  (`StageConstants::compute`, `StageDemand::compute`), builds its rule set
  around the result, and runs through `Program::run_pass`. Nothing is cached
  and no revision is checked.

## Run

From the repository root:

```sh
cargo bench -p toy-lang --bench incremental -- \
  --sizes 10 100 300 --repeats 5 --warmups 1 --json /tmp/incremental.json
```

Defaults are both shapes, sizes 10 and 100, 8 units per function, every case,
3 rounds, 10 repeats and 2 warmups. `--per-pass` prints each pass of the median sample.
`call_chain` grows quadratically; size 300 takes minutes.

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
| `clean` | none |

The `edit_*` cases are a recompile after editing one function: every other
function is already folded. `clean` is a recompile with nothing to do.

## Pipeline

`--rounds` rounds of `fold` then `free`; the last round must edit nothing.
Rounds after the fixpoint stand for later passes of a fixed pipeline that
find nothing to do, and are where `database` reuses results. Each pass is one
`run_pass` running a `RewriteDriver` with one rule from
`toy_lang::analysis_clients`:

- `fold` reads stage-wide constprop (`ConstPropFacts`) and replaces each
  constant `i64` `arith` result with a new `constant`.
- `free` reads stage-wide demand (`DemandFacts`) and moves the uses of each
  undemanded `arith` result onto a new `constant 0`.

The driver's dead-code cleanup erases what the rules leave unused. Both
analyses root every function of the stage, with unknown arguments (constprop)
or a demanded return (demand), and join every root's facts into one map, so
a rule's read is one lookup.

## Columns

| Column | Meaning |
| --- | --- |
| `total` | the sum of every pass: its analyses, building its rule set, and `run_pass` |
| `analysis` | analyses computed for the passes' requirements |
| `affected` | the measured time of the roots that reach the edited function, summed over every analysis run (`edit_*` only). The set comes from the workload's call structure, not from the results |
| `driver` | the `RewriteDriver` runs |
| `lookup` | the rules' reads of analysis results, inside `driver` |
| `overhead` | `total - analysis - driver`: building the rule set, stage take/restore, and `verify_derived` |
| `runs`, `hits` | analysis runs, and passes whose analysis was cached |
| `roots` | roots analyzed over all runs |
| `lookups` | rule reads of analysis results |
| `applied`, `erased`, `sweeps` | the drivers' `RewriteSummary`, summed |

Times are medians over the repeats, each on freshly parsed IR. Parsing and
the checks are outside the timer.

## Checks

On a first, untimed run of every row: `@f0`'s concrete result at three
inputs is the same before and after, the program has `units` fewer
statements per dirty function, `fold` applied `units` times and `free` once
per dirty function, and the last round edited nothing. A row
that fails prints `FAIL` and the benchmark exits nonzero.

## Instrumentation

`toy_lang::analysis_clients::log` records, per thread, each analysis run with
the time of each root and the time and count of the rules' reads. The
benchmark reads it around each pass.
