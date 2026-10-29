# Constant-propagation measurements — 26 October 2026

Measured Rust source: `83d499be3dff7d8e30120570538e32d116118303`. Python source: `eae74008d4aec630169b5e64cc085e6f2c0d5b12`.

Host: Apple M5 Pro; 48 GiB RAM; 18 logical CPUs; macOS-26.6.2-arm64-arm-64bit. Python: Python 3.14.5 (main, May 10 2026, 19:20:57) [Clang 22.1.3 ]. Rust: rustc 1.96.0 (ac68faa20 2026-05-25), release build.

The full cached matrix covers seven workloads at sizes 1,000, 10,000, and 100,000. Each pair checks 68 abstract inputs and times three configurations with five warmups and 30 measured repetitions: **1,428 check rows per implementation and 3,780 measured samples across both implementations**. All samples and checks are retained in [cached.json](cached.json).

Each timed repetition constructs a fresh analyzer and analyzes already-built native IR. IR construction, argument preparation, JSON communication/reporting, correctness checks, and final analyzer/result/cache destruction are excluded. Python call caching is enabled in the main run, starts empty each repetition, and only reuses calls within that analysis.

## Cached timings at size 100,000

All inputs known; median milliseconds.

| Workload | Rust (ms) | Python (ms) |
| --- | ---: | ---: |
| Chain | 20.654 | 429.679 |
| Diamond | 10.177 | 220.783 |
| Nested branches | 5.377 | 108.614 |
| Loop | 81.893 | 699.738 |
| Chained calls | 2791.194 | 2021.761 |
| Repeated identical calls | 160.826 | 856.315 |
| Recursion | 0.582 | 30.201 |

## Correctness and precision

Full cached matrix, all three sizes:

| Implementation | Pass | Imprecise | Unsound | Error |
| --- | ---: | ---: | ---: | ---: |
| Rust | 1362 | 66 | 0 | 0 |
| Python | 1380 | 0 | 48 | 0 |

Verdicts are recorded rather than enforced. Rust's function-context budget and Python's recursion cutoff affect precision. Chained-call and recursion timings do not establish equivalent-result speedups. See the per-input results and verdicts in [cached.json](cached.json) and the [verdict definitions](../../README.md#checks).

## Python call-cache comparison at size 1,000

[uncached-1000.json](uncached-1000.json) contains a separate run of calls, calls_same, and recursion with caching disabled, using the same sources, five warmups, and 30 measurements. It includes all 204 check rows per implementation and all measured samples.

| Workload / inputs | Uncached (ms) | Cached (ms) | Uncached / cached |
| --- | ---: | ---: | ---: |
| calls / all_known | 16.911 | 18.619 | 0.91× |
| calls / all_unknown | 14.034 | 3.386 | 4.14× |
| calls_same / all_known | 21.344 | 7.830 | 2.73× |
| calls_same / all_unknown | 16.509 | 5.996 | 2.75× |

## Reproduce

Check out the measured Rust and Python commits above. From the Rust repository root, with the Python checkout's environment installed:

```sh
python3 benchmarks/constprop/compare.py \
  --python-checkout /path/to/python-kirin --python-cache-calls \
  --output /path/to/cached.json

python3 benchmarks/constprop/compare.py \
  --python-checkout /path/to/python-kirin \
  --families calls calls_same recursion --sizes 1000 \
  --output /path/to/uncached-1000.json
```

The raw reports include source revisions, imported Python source, workload-specification hash, hardware/toolchain metadata, all abstract-result checks, raw timing samples, medians, and ratios. These are single-host measurements of the selected implementations and configurations.
