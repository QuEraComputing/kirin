# Constant-propagation measurements — 23 October 2026

Source: `0bac36b5eea4aaad004eee4936db5a697ac3cd0d`; Python: `eae74008d4aec630169b5e64cc085e6f2c0d5b12`. Apple M5 Pro, 48 GiB RAM, macOS 26.6.2, Rust 1.96.0 release, Python 3.14.5. Five warmups and 30 measured repetitions per input configuration. Python call caching is enabled unless marked uncached; each repetition starts with a fresh analyzer/cache.

The corrected harness excludes argument preparation and final analyzer/cache/result destruction on both sides. Earlier measurements from before this fix are not the uncached control used here.

## Cached timings at size 100,000

All inputs known; median milliseconds. Recursion is a separate recovered run.

| Workload | Rust | Python with call caching |
| --- | ---: | ---: |
| chain | 20.655 | 420.543 |
| diamond | 10.554 | 217.521 |
| nested_cfg | 5.387 | 106.196 |
| loop | 81.031 | 669.365 |
| calls | 2852.281 | 2008.654 |
| calls_same | 158.468 | 926.560 |
| recursion | 0.591 | 30.807 |

Interpret call and recursion timings alongside precision limits: Rust uses a 64-context budget per function; Python has a recursion cutoff. These timings are not evidence that both implementations infer equally precise answers.

## Cache effect at size 1,000

Both runs use the corrected harness. Values are Python median milliseconds.

| Workload / inputs | Uncached | Cached | Uncached / cached |
| --- | ---: | ---: | ---: |
| calls / all_known | 17.109 | 19.555 | 0.87× |
| calls / all_unknown | 13.938 | 3.391 | 4.11× |
| calls_same / all_known | 21.664 | 7.573 | 2.86× |
| calls_same / all_unknown | 16.747 | 6.025 | 2.78× |

The complete size-1,000 cached check matrix has 476 rows per implementation: Rust has 454 passes and 22 imprecise results; Python has 460 passes and 16 unsound results, all from recursion. The corrected uncached comparison covers only calls, calls_same, and recursion (204 rows per implementation); its check results match the cached run exactly.

## Data and recovery

- [Cached size-1,000 raw samples and checks](cached-recovered-1000.json).
- [Corrected uncached size-1,000 call/recursion raw samples and checks](uncached-corrected-1000.json).
- [Recovered size-100,000 recursion raw samples and checks](cached-recovered-recursion-100000.json).
- [All 63 cached medians](cached-medians.json).
- [Original interrupted run log](cached-interrupted.log).

The full run completed 20 of 21 workload/size pairs before another task switched the shared checkout, removing the benchmark scripts before the final Python recursion subprocess could start. The driver writes its combined report only at the end, so the raw samples/check rows for those 20 pairs were lost. Their medians survive in the log, rounded to 0.001 ms. No full-suite verdict count is claimed.

The complete size-1,000 matrix and final recursion pair were rerun using `git archive` of the measured commit in a temporary directory and a copy of the same release executable; no new Git worktree was created. The raw recovered reports identify the source commit and executable SHA-256. Their environment metadata comes from the corrected uncached run on the same host.

To reproduce from the measured Rust commit with the specified clean Python checkout:

```sh
python3 benchmarks/constprop/compare.py \
  --python-checkout /path/to/python-kirin --python-cache-calls \
  --output /path/to/cached.json

python3 benchmarks/constprop/compare.py \
  --python-checkout /path/to/python-kirin \
  --families calls calls_same recursion --sizes 1000 \
  --output /path/to/uncached-1000.json
```
