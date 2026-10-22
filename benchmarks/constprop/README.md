# Constant-propagation benchmark

Runs Kirin-Rust's `ConstProp` and Kirin-Python's `const.Propagate` on the
shared workloads in [`workloads.json`](../json_roundtrip/workloads.json), built
by the JSON round-trip benchmark's builders. It checks every result and times
both.

## Run

From the Rust repository root, with a clean Python checkout that has a `.venv`
(`uv sync --no-dev`):

```sh
python3 benchmarks/constprop/compare.py \
  --python-checkout /absolute/path/to/python-kirin \
  --output /absolute/path/to/results.json
```

Defaults are every family, sizes 1000, 10000 and 100000, 30 repeats and 5
warmups.

| Option | Effect |
| --- | --- |
| `--sizes 4 --repeats 3 --warmups 1` | quick run |
| `--families` | select workloads |
| `--python` | select another interpreter |
| `--python-cache-calls` | run Python with `cache_calls=True` |

As in the JSON benchmark, the driver checks that the interpreter imports
`kirin` from the checkout.

## Workloads

Every family is one `@benchmark(x, y, outer, inner)` function, scaled by
`size`. Some families add one callee.

| Family | Shape |
| --- | --- |
| `chain`, `diamond`, `nested_cfg` | straight-line and branching arithmetic; see the [JSON benchmark](../json_roundtrip/README.md) |
| `loop` | CFG loop; `outer` chooses 4 or 0 iterations, and each iteration adds `y` `size` times |
| `calls` | `size` chained calls `acc = step(acc, y)` |
| `calls_same` | `size` calls `step(x, y)` with the same arguments, each result added to `x` |
| `recursion` | `rec(depth, x, y)` adds `y` at each level; `outer` chooses depth `size` or 0 |

`calls` and `recursion` reach each implementation's precision limits: Rust's
per-function context budget and Python's recursion cutoff. Probe the limits
with `--families calls recursion --sizes 16 64 128 1024`.

## Inputs

An abstract input sets each of `x`, `y`, `outer` and `inner` to a sample value
or to unknown (`null`, ⊤). The seven samples under all 16 known/unknown masks
give 68 distinct inputs. Each one is analyzed for every family and size. Three
of them are also timed:

| Config | `x`, `y` | `outer`, `inner` |
| --- | --- | --- |
| `all_known` | sample 0 | sample 0 |
| `unknown_conditions` | sample 0 | ⊤ |
| `all_unknown` | ⊤ | ⊤ |

## Checks

Each result is projected to an integer constant, `"top"`, `"bottom"`, `"other"`
(any partial value) or `"error"` (the analysis failed), then judged:

- **error**: the analysis failed.
- **unsound**: the result is not `"top"`, and the closed-form oracle gives a
  different value at some concrete point matching the input. Unknown integers
  range over the declared domain (|v| ≤ 8); unknown Booleans take both values.
  `"bottom"` and `"other"` are always unsound.
- **pass**: the result equals the target.
- **more_precise**: the result is sound and constant, but the target is
  `"top"`.
- **imprecise**: the result is sound but `"top"`, where the target is a
  constant.

The target is what flat constant propagation should reach:

1. The feasible paths are those that agree with the known Booleans.
2. If a feasible path reads an unknown input, the target is `"top"`. A path
   reads an input when the input's coefficient is nonzero.
3. If a looping path (`trips > 0`) changes its carried value in an iteration,
   the target is `"top"`. A loop joins its carried value across iterations, so
   it stays constant only if no iteration changes it.
4. Otherwise the target is the paths' common value if they all agree, else
   `"top"`.

Calls and recursion are treated as transparent. A depth or context limit
therefore shows up as an `imprecise` or `unsound` verdict.

Verdicts are recorded and counted, not enforced.

## Timed boundary

- Rust: `ConstProp::new(&ir)`, then `analyze_by_name`.
- Python: `const.Propagate(dialects)`, then `.run(method, *args)`.

Building IR, checking, and dropping the analyzer and its result are outside the
timer. Python collects garbage between samples and disables it during each
sample.

## Report

`results.json` holds:

- the environment (as in the JSON benchmark, plus the Python revision)
- the workload spec hash and the `cache_calls` setting
- verdict counts for each implementation
- for each family and size, every check row and the timing rows (all samples,
  medians, and the Python/Rust ratio)
