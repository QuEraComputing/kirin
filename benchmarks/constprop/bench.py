"""Python half of the constprop benchmark. Run with the checked-out Python's interpreter."""
import gc
import json
from pathlib import Path
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import kirin
from kirin.analysis import const
from common.python_workload import workload


def analyze(program, input, options):
    """Returns (frame, result); a failed analysis returns (None, error)."""
    args = [const.Unknown() if input[key] is None else const.Value(input[key])
            for key in ("x", "y", "outer", "inner")]
    try:
        return const.Propagate(program.dialects, **options).run(program, *args)
    except Exception as error:
        return None, error


def project(result):
    if isinstance(result, Exception):
        return "error"
    if isinstance(result, const.Value) and type(result.data) is int:
        return result.data
    if isinstance(result, const.Unknown):
        return "top"
    if isinstance(result, const.Bottom):
        return "bottom"
    return "other"


def measure(program, input, options, warmups, repeats):
    for _ in range(warmups):
        analyze(program, input, options)
    samples = []
    for _ in range(repeats):
        # Collect earlier frames between samples, not inside the timer.
        gc.collect()
        gc.disable()
        start = time.perf_counter()
        output = analyze(program, input, options)
        samples.append(time.perf_counter() - start)
        gc.enable()
        del output
    return samples


def main():
    request = json.load(sys.stdin)
    program = workload(request["family"], request["size"])
    # Older Python checkouts have no `cache_calls`, so pass it only when asked.
    options = {"cache_calls": True} if request["cache_calls"] else {}
    check = [project(analyze(program, input, options)[1]) for input in request["check"]]
    timed = [{"result": project(analyze(program, input, options)[1]),
              "samples_seconds": measure(program, input, options, request["warmups"], request["repeats"])}
             for input in request["time"]]
    print(json.dumps({"kirin_source": str(Path(kirin.__file__).resolve()), "check": check, "time": timed}))


if __name__ == "__main__":
    main()
