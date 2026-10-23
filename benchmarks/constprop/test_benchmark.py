"""Run with the benchmark's Python checkout: unittest discover -s this directory."""
import gc
import unittest
import weakref
from unittest.mock import patch

import bench
from compare import judge, target
from common.semantic_spec import expected_cases, family_spec
from kirin.interp.concrete import Interpreter


class ConstpropBenchmark(unittest.TestCase):
    def test_new_workloads_execute_against_independent_oracle(self):
        for family in ("loop", "calls", "calls_same", "recursion"):
            program = bench.workload(family, 4)
            program.code.verify()
            program.code.verify_type()
            for case in expected_cases(family, 4):
                with self.subTest(family=family, input=case["input"]):
                    args = [case["input"][key] for key in ("x", "y", "outer", "inner")]
                    result = Interpreter(program.dialects).run(program, *args)[1]
                    self.assertEqual(result, case["expected"])

    def test_analyzer_and_cache_survive_until_timer_stops(self):
        program = bench.workload("calls_same", 4)
        args = bench.arguments(dict(x=2, y=3, outer=True, inner=True))
        propagate = bench.const.Propagate
        for cache_calls in (False, True):
            references = []
            ticks = []

            def construct(*args, **kwargs):
                analysis = propagate(*args, **kwargs)
                references.append(weakref.ref(analysis))
                return analysis

            def clock():
                if ticks:
                    analysis = references[0]()
                    self.assertIsNotNone(analysis, "analyzer was destroyed inside timing")
                    self.assertEqual(bool(analysis._call_summaries), cache_calls)
                ticks.append(len(ticks))
                return ticks[-1]

            with self.subTest(cache_calls=cache_calls), \
                 patch.object(bench.const, "Propagate", side_effect=construct), \
                 patch.object(bench.time, "perf_counter", side_effect=clock):
                self.assertEqual(bench.measure(program, args, {"cache_calls": cache_calls}, 0, 1), [1])
            gc.collect()
            self.assertIsNone(references[0]())

    def test_gc_state_is_preserved(self):
        program = bench.workload("chain", 4)
        args = bench.arguments(dict(x=2, y=3, outer=True, inner=True))
        enabled = gc.isenabled()
        gc.disable()
        try:
            bench.measure(program, args, {}, 0, 1)
            self.assertFalse(gc.isenabled())
        finally:
            if enabled:
                gc.enable()

    def test_verdicts_distinguish_wrong_answers_from_precision_loss(self):
        family = family_spec("calls", 4)
        known = dict(x=2, y=3, outer=True, inner=True)
        results = {"correct": 14, "wrong": 15, "lost": "top", "bottom": "bottom", "failed": "error"}
        row = judge(family, 4, known, results, 8)
        self.assertEqual([row[key]["verdict"] for key in results],
                         ["pass", "unsound", "imprecise", "unsound", "error"])
        unknown = dict(known, x=None)
        self.assertEqual(judge(family, 4, unknown, {"constant": 14}, 8)["constant"]["verdict"], "unsound")

    def test_loop_target_accounts_for_skipped_and_stable_loops(self):
        family = family_spec("loop", 4)
        self.assertEqual(target(family, 4, dict(x=2, y=None, outer=False, inner=None)), 2)
        self.assertEqual(target(family, 4, dict(x=2, y=0, outer=None, inner=None)), 2)
        self.assertEqual(target(family, 4, dict(x=2, y=3, outer=True, inner=None)), "top")


if __name__ == "__main__":
    unittest.main()
