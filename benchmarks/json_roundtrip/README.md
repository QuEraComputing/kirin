# JSON round-trip benchmark suite

This folder contains the common semantic specifications, separate Rust/Python
payload builders, real-runtime validation, timing driver, and regression tests.
The benchmark measures **complete IR → JSON string → usable IR**. It does not
time execution of the arithmetic programs.

## Inspect the suite

| File | Purpose |
| --- | --- |
| `workloads.json` | Common workload paths, seven sample inputs, bounded domain, and independent closed-form oracle coefficients |
| `rust/workload.rs` | Rust-specific native text builder |
| `rust/language.rs` | Native arithmetic/CFG language and dispatch to Kirin's standard interpreter rules |
| `rust/semantics.rs` | Executes original and decoded Rust payloads in `ConcreteInterpreter` against the oracle |
| `python_benchmark.py` | Builds Python `Method`/block/SSA objects directly and measures Python JSON round trips |
| `semantic_spec.py` | Reads the common specification, evaluates the closed-form oracle, and executes Python's real `Interpreter` |
| `rust/shape.rs`, `validation.py` | Local IR validity checks; shapes are diagnostics, not a cross-version equality requirement |
| `compare.py` | Builds Rust release binary, runs both implementations, accepts sampled semantic agreement, and reports timings/environment/spec hash |
| `rust/tests.rs`, `test_workloads.py` | Original/decoded execution and validity regressions, including deliberately incorrect arithmetic |

There is no shared text parser. Rust generates its native text and parses it
once into a finalized pipeline before timing. Python constructs its native IR
objects directly, without Python-source lowering or optimization passes. Both
builders use the common path specification, then serialize their own complete
native IR payloads. Python's required Method/self metadata has no decorative
counterpart in Rust. The JSON schemas, object layouts, operation metadata,
operation counts, block counts, serialized sizes and diagnostic shape hashes
may differ.

## Semantic workload specifications

`size` is the representative arithmetic payload scale. The current builders
use `size` arithmetic operations spread across the paths, but matching native
operation counts is not a qualification condition. Let `K` be the number of
steps on a selected path:

| Workload | Steps on each path | Expected result |
| --- | --- | --- |
| `chain` | `K = size` | `x + K*y` |
| `diamond` | `K = size/2` | `outer ? x + K*y : x - K*y` |
| `nested_cfg` | `K = size/4` | See the four independent Boolean paths below |

| `outer` | `inner` | Nested path | Expected result |
| --- | --- | --- | --- |
| true | true | a | `x + K*y` |
| true | false | b | `x - K*y` |
| false | true | c | `(K + 1)*y` |
| false | false | d | `y - K*x` |

The manifest separates builder instructions (`start`, `delta`, `operation`)
from expected-result coefficients. The oracle computes a linear closed form;
it does not replay the generated operation chain. Seven sample inputs include
all four Boolean combinations with `x=2,y=3`, negative values, and zero values.
For `size=4`, the four nested paths return **5, -1, 6, 1**, so swapping branches
or accidentally reusing one condition is observable. Every specified path must
be covered by the sample set.

The declared domain is sizes divisible by four from 4 through 100,000,
`|x|, |y| <= 8`, and two independent Booleans. Current sample inputs satisfy
these bounds, and all intermediate/final values fit `i64`. Python uses its
unbounded `Int`; Rust uses `i64`. Static Boolean types are preserved, while the
Rust concrete interpreter's `i64` value domain supplies `0/1` for false/true.
These are intentional representation differences.

For every family/size and sample input, both real interpreters execute:

1. The original IR payload.
2. The IR reconstructed from JSON.

Both outputs must equal the same independent expected result. Python also
runs `code.verify()` and `code.verify_type()` before/after decoding, and checks
`is_structurally_equal`. Rust checks printed text, all encoded fields, SSA/CFG
metadata, function/entry signatures, branch parameter signatures, Boolean
conditions, and integer arithmetic/returns. Local CFG checks also cover
terminator placement, SSA definition uniqueness, dominance and definition
order. None of this validation is timed.

This is **representative bounded semantic validation**, not a universal proof
of interpreter equivalence. Shape equality is unnecessary across versions;
local pre/post shape preservation remains useful for serialization correctness.
Reports retain each sample input, path, expected/original/decoded output,
workload specification SHA-256, and separate representation diagnostics.

The small corpus currently omits loops. A future Python iterable-based loop
and Rust start/end/step loop may be compared when both satisfy the same finite
semantic specification; identical loop representation is not required.
Structured conditionals/loops and graph bodies remain covered by the separate
Rust JSON correctness corpus.

## Run the comparison

Use a clean Python checkout pinned at
`eae74008d4aec630169b5e64cc085e6f2c0d5b12` (the cached `origin/main` when the
suite was prepared). In that checkout:

```sh
git checkout --detach eae74008d4aec630169b5e64cc085e6f2c0d5b12
uv sync --no-dev
```

From the Rust repository root:

```sh
python3 benchmarks/json_roundtrip/compare.py \
  --python-checkout /absolute/path/to/python-kirin \
  --sizes 1000 10000 100000 --repeats 30 --warmups 5 \
  --output /absolute/path/to/results.json
```

For small, directly inspectable payloads and a fast validation run:

```sh
python3 benchmarks/json_roundtrip/compare.py \
  --python-checkout /absolute/path/to/python-kirin \
  --sizes 4 --repeats 3 --warmups 1 \
  --export-payloads /absolute/path/to/payloads \
  --output /absolute/path/to/results.json
```

Exports are written outside timing: each case produces `.rust.kirin`,
`.python.kirin`, `.rust.json`, and `.python.json`. The Python text is a plain
dump from its native IR printer for human comparison, without requiring a text
parser. `--python` can select another interpreter
with the pinned Python source installed. Before building or timing, the driver
checks that this interpreter actually imports `kirin` from the checked baseline,
rejecting conflicting `PYTHONPATH` entries or editable installs. It records the
resolved source path and checks it again in each measuring subprocess. The driver
builds Rust in release mode, runs both on the same host, alternates implementation order across cases,
and records revisions, dirty Rust files, compiler/Python/platform/hardware,
all samples, medians, serialized sizes and Python/Rust median ratios. Smoke
ratios validate the harness; they are not performance thresholds or QBI results.

## Exact timed boundary

- Rust: `serde_json::to_string(&pipeline)`, then
  `serde_json::from_str::<Pipeline<_>>`.
- Python: `basic.encode_json(method)`, then `basic.decode_json(payload)`,
  including semantic serialization-unit conversion and usable IR reconstruction.

Construction, parsing, printing, real-interpreter execution, oracle checks,
structural/type validation, I/O, process startup, compilation and reporting are
outside the timer. Both outputs remain alive until after timing stops. Final
JSON/IR destruction is excluded. Python collects prior cyclic IR objects
between samples and disables cyclic GC during each sample; allocations and
reference-count work internal to encoding/decoding remain timed. Five warmups
and thirty measured repetitions are the defaults.

## Regression tests and JSON storage support

```sh
cargo test --example json_roundtrip --features serde
/absolute/path/to/python-kirin/.venv/bin/python -m unittest discover \
  -s benchmarks/json_roundtrip -p 'test_*.py'
cargo test --test roundtrip --test json_storage --features serde
cargo test -p toy-qc --features serde
```

Fixture tests execute original/decoded payloads against the oracle and prove
that structurally valid but incorrect arithmetic is rejected. Representation
shape differences are explicitly accepted when semantic observations agree.
Malformed types, branch arities and SSA dominance are also tested locally.

Enable `serde` on `kirin` and each built-in dialect crate; derive Serialize/
Deserialize on the language/type/value definitions and heterogeneous stage enum
if used. Finalized `StageInfo` and full `Pipeline` preserve arena IDs/tombstones,
symbols, operation attributes, SSA/CFG links and metadata, graph connectivity,
function/stage/specialization links, invalidation flags and backedges. Symbol
and function-name lookup indexes are reconstructed on decode. `ArithValue`
floats use exact IEEE bits, including signed zero/nonfinite values/NaN payloads;
128-bit integers use decimal strings. Downstream dialects own exact attribute
encoding (a plain serde-derived nonfinite `f64` does not survive serde_json).
This is native storage with compatible dialect definitions/schema, not a
versioned Rust/Python interchange format; deserialization alone is no verifier.

## Recorded local run and plot

The 15 October 2026 run is stored in `results/2026-10-15/roundtrip.json`,
including all 30 samples per implementation/case, five warmups, semantic
checks, environment, and measured revisions. `roundtrip.png` plots the
medians with interquartile ranges, using common logarithmic axes. Native
JSON formats differ, so the speedups describe these complete serializers
on representative payloads on one host. The measured Rust revision is
`dc3d2362021927633098cceb0d617f434d1dbfc5`; subsequent provenance checks
and print exports do not change the timed serialization code.

To regenerate the static plot in an environment with `matplotlib==3.10.0`:

```sh
python benchmarks/json_roundtrip/plot_results.py \
  benchmarks/json_roundtrip/results/2026-10-15/roundtrip.json \
  benchmarks/json_roundtrip/results/2026-10-15/roundtrip.png
```
