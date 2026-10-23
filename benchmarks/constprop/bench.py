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


def arguments(input):
    return [const.Unknown() if input[key] is None else const.Value(input[key])
            for key in ("x", "y", "outer", "inner")]


def analyze(program, args, options):
    """Keep the analyzer and its call cache alive until after timing stops."""
    analysis = None
    try:
        analysis = const.Propagate(program.dialects, **options)
        return analysis, analysis.run(program, *args)
    except Exception as error:
        return analysis, (None, error)


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


def measure(program, args, options, warmups, repeats):
    for _ in range(warmups):
        analyze(program, args, options)
    samples = []
    for _ in range(repeats):
        # Collect earlier frames between samples, not inside the timer.
        gc.collect()
        enabled = gc.isenabled()
        gc.disable()
        try:
            start = time.perf_counter()
            output = analyze(program, args, options)
            samples.append(time.perf_counter() - start)
        finally:
            if enabled:
                gc.enable()
        del output
    return samples


def main():
    request = json.load(sys.stdin)
    program = workload(request["family"], request["size"])
    # Older Python checkouts have no `cache_calls`, so pass it only when asked.
    options = {"cache_calls": True} if request["cache_calls"] else {}
    check = [project(analyze(program, arguments(input), options)[1][1]) for input in request["check"]]
    timed = [{"result": project(analyze(program, args, options)[1][1]),
              "samples_seconds": measure(program, args, options, request["warmups"], request["repeats"])}
             for args in map(arguments, request["time"])]
    print(json.dumps({"kirin_source": str(Path(kirin.__file__).resolve()), "check": check, "time": timed}))


if __name__ == "__main__":
    main()
