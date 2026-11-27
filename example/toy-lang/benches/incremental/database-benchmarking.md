# Analysis database benchmark: findings

Branch `alex/analysis-db-bench` (local), on top of `alex/analysis-manager`.
The benchmark and how to run it are in [README.md](README.md).

## What is measured

A generated toy-lang program of N functions. A function is *dirty* if it
has work for the passes: arithmetic on constants (`a = 2; b = a + a`) and a
value (`d = x * x`) passed to a callee that never reads it. A *clean* function
has the same number of statements, none of which either pass can change: its
arithmetic depends on its input (`a = x - x`), and it passes `x`. Two shapes:

- **separate**: every function calls one shared leaf.
- **chained**: `f0` calls `f1`, `f1` calls `f2`, and so on.

The pipeline is a fixed list of 6 passes, `fold, free, fold, free, fold,
free`:

- **fold** reads constant propagation and replaces constant results with
  constants.
- **free** reads demand (strong liveness) and removes values nothing needs.

Both analyses are stage-wide: they analyze every function as an entry point.
The first two passes do all the work, and the last four find nothing.

Each case runs two ways:

- **database**: passes run through `Database::run_pass`. A result is reused
  until a pass edits the program, and any edit drops every result.
- **no database**: each pass computes its analysis itself, then runs. Nothing
  is cached and no revision is checked.

The cases:

| Case | Program given to the compiler |
| --- | --- |
| first compile | every function dirty |
| edit `f0` | only `f0` dirty, as if just edited |
| edit last | only the last function dirty |
| nothing to do | every function clean |

## Results

Times are medians of 3 runs on a release build, for all 6 passes.
"Needed" is the measured analysis time of only the functions whose analysis
reaches the edited function. It shows how much of the re-analysis an edit
actually required. That set comes from the program's known call structure,
not from the analysis results.

| Program | Case | Total, no database (ms) | Total, database (ms) | Analysis runs, no database / database | Analysis time, database (ms) | Needed (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| 1,000 separate | first compile | 89 | 73 | 6 / 4 | 35 | – |
| 1,000 separate | edit `f0` | 108 | 89 | 6 / 4 | 40 | 0.1 |
| 1,000 separate | nothing to do | 107 | 67 | 6 / 2 | 19 | – |
| 300 chained | first compile | 1,788 | 1,210 | 6 / 4 | 1,199 | – |
| 300 chained | edit `f0` | 2,155 | 1,425 | 6 / 4 | 1,411 | 10 |
| 300 chained | edit last | 2,174 | 1,425 | 6 / 4 | 1,410 | 1,399 |
| 300 chained | nothing to do | 2,155 | 723 | 6 / 2 | 708 | – |

## Findings

### The database saves only passes that follow passes with no edits

Every saving in the table comes from skipped analysis runs: 6 against 4 when
the program needs work, 6 against 2 when it doesn't. The two passes that edit
drop the cache. Pass 3 recomputes constprop, and pass 4 recomputes demand,
since only constprop is cached at that point. Passes 5 and 6 reuse those
results. A pipeline that stops at the first round with no edits never reuses
anything, and with it the two modes time the same.

### Analysis dominates, and an edit re-analyzes everything

Analysis is about half the time on separate functions and 99% on chained
ones. After editing one function, every function is re-analyzed:

- 1,000 separate functions: 40 ms re-analyzed, 0.1 ms needed.
- 300 chained functions, editing `f0`: 1,411 ms re-analyzed, 10 ms needed.
- 300 chained functions, editing the last: 1,410 ms re-analyzed, 1,399 ms
  needed, since every function's analysis reaches it.

### Stage-wide roots repeat work across callers

Analyzing `f_i` as an entry point also analyzes everything it calls, so in a
chain each function's body is analyzed once per caller above it. Analysis
time grows quadratically: in a first compile with the database, 131 ms for
100 functions and 1,199 ms for 300 (3× the functions, 9× the time).

### Per-pass overhead is the next cost on separate functions

Taking the stage out and back, verifying it, and building the rule set take
20–36 ms of the 1,000-function runs, about as much as the rewrites (13–18 ms).
It has not been broken down further.

## Caveats

- The functions are 26 straight-line statements with no loops, so each
  analysis converges in one visit. Real code would make analysis costlier
  and the gap larger.
- "Needed" ignores the cost of tracking which functions an analysis reached
  and storing results per function.

## Implications for per-function invalidation

- Keeping the results of functions an edit does not reach would remove most
  re-analysis after small edits (40 ms to 0.1 ms above), but not when the
  edited function is reached by most of the program.
- It would not remove the repeated callee work of stage-wide roots. Reusing
  each callee's result across its callers would.
