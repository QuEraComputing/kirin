# JSON IR round trips and Python comparison

Enable `serde` on `kirin` and on each built-in dialect crate in a language.
Derive `serde::Serialize` and `serde::Deserialize` on the language enum and
its type/value enums (and on a heterogeneous stage enum, if used). A finalized
`StageInfo<L>` or complete `Pipeline<S>` then works with `serde_json::to_string`
and `serde_json::from_str` directly. No text parsing is used by the serializer.

The storage format preserves arena positions, deleted slots, next allocation
IDs, local/global symbol identities, operation definitions/attributes, types,
SSA links and reverse-use metadata, block/CFG links and predecessors, directed
and undirected graph connectivity/ports, and function/stage/specialization
links, invalidation flags and backedges. Intern tables encode their ordered
items and rebuild their hash indexes; pipelines rebuild the function-name
index on decoding. Duplicate interned entries/function names are rejected.

Built-in `ArithValue` floats encode their IEEE bits as integers, preserving
signed zero, infinities and NaN payloads. Its 128-bit integers encode decimal
strings, so the complete range also survives JSON object/value intermediates.
Downstream dialects own their attribute serialization; a plain serde-derived
`f64` field does not preserve nonfinite values through `serde_json`. Use an
exact encoding for such fields when those values belong to the dialect.

This is a Rust IR storage representation, not Python's JSON schema or a
versioned interchange format. Decode using compatible dialect/type definitions
and the same storage schema. The round-trip tests check all encoded fields,
printed IR and derived metadata; deserialization alone is not an IR verifier.

## Correctness corpus

```sh
cargo test --test roundtrip --test json_storage --features serde
cargo test -p toy-qc --features serde
```

With serde enabled, the shared round-trip assertions additionally perform
text → IR → JSON → IR → text, compare all encoded fields, and verify SSA/CFG
metadata. The corpus includes attributes/constants, numeric types, multiple
results, branches and block arguments, functions/calls/captures, nested
conditionals/loops, directed forward references and graph captures, and all
four body kinds. The toy-qc file corpus includes quantum circuits and an
undirected ZX graph with explicit wire edges. Storage regression tests cover
tombstones, index reconstruction, multi-stage links and extreme numeric values.

## Reproduce the performance comparison

Use the Python baseline at exactly
`eae74008d4aec630169b5e64cc085e6f2c0d5b12` (the cached `origin/main` when this
benchmark was prepared), in a separate clean checkout. In that checkout:

```sh
git checkout --detach eae74008d4aec630169b5e64cc085e6f2c0d5b12
uv sync --no-dev
```

From the Rust checkout:

```sh
python3 benchmarks/json_roundtrip/compare.py \
  --python-checkout /absolute/path/to/python-kirin \
  --sizes 1000 10000 100000 --repeats 30 --warmups 5 \
  --output /absolute/path/to/results.json
```

The driver builds the Rust example in release mode, runs both implementations
on the same host, alternates implementation order across cases, and records
the revisions, dirty Rust files, compiler/Python/platform/processor versions,
all samples, median latency, serialized sizes and Python/Rust median ratios.
`--python` can select a different interpreter with the baseline installed.
For a fast qualification run use `--sizes 4 1000 --repeats 3 --warmups 1`.

### Equivalent workloads

The size is the number of **add operations**, not total operation count.
Inputs are integers; each chain repeatedly adds its first input to its
previous result. No compiler/optimization pass runs during construction.

| Family | Construction | Other operations | Blocks |
| --- | --- | --- | --- |
| `chain` | One chain containing N adds | 1 return | 1 |
| `diamond` | Conditional branch to two N/2-add chains, then merge | 1 conditional branch, 2 branches, 1 return | 4 |
| `nested_if` | Two levels of conditionals; four N/4-add leaf chains | 3 ifs, 6 yields, 1 return | 7 |

Both have an unused first entry argument (Python Method's implicit self),
followed by integer `x` and integer `condition` arguments. Python uses `Int`;
Rust uses `i64`. The condition uses integer truthiness. The workloads have the
same mathematical semantics for inputs whose results fit `i64`; this is not
an arbitrary-precision arithmetic performance comparison.

Before accepting a comparison, the driver requires equality of a normalized
**actual IR** description: every block argument and result's integer type,
every operation and operand/result SSA identity, CFG successor, and nested
body reference. Both implementations also check this description after JSON
decoding. Rust checks its final text against the text printed before JSON;
Python checks `is_structurally_equal`. Schemas, metadata overhead and JSON
sizes may differ. The report stores a SHA-256 of the matching description.
Graph-specific correctness remains in the Rust corpus because Python has no
matching undirected graph body representation for this benchmark.

### Timed boundary

Only **complete IR → JSON string → complete usable IR** is timed:

- Rust: `serde_json::to_string(&pipeline)` then `serde_json::from_str::<Pipeline<_>>`.
- Python: `basic.encode_json(method)` then `basic.decode_json(payload)`, including
  all semantic serialization-unit conversion and IR reconstruction.

Text generation/parsing/printing, construction, equivalence checks, operation
counting, process startup, disk I/O, compilation, reporting and final destruction
of the returned JSON/IR are outside the timer. The benchmark holds both outputs
until after the timer stops. Python collects previous cyclic IR objects between
samples and disables cyclic GC during each timed sample; ordinary allocations
and reference-count work inside serialization/deserialization remain timed.
Five warm-ups precede thirty measured repetitions by default. Smoke-run ratios
validate the harness; they are not a qualification threshold or a QBI result.
