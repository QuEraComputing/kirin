"""Run with the pinned baseline Python: -m unittest discover -s this_directory."""
import copy
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kirin.dialects.py.binop import Add, Sub
from kirin.prelude import basic
from python_benchmark import shape
from common.python_workload import workload
from common.semantic_spec import expected_cases
from semantics import assert_semantically_valid, validate_runtime
from validation import validate_shape


class WorkloadValidity(unittest.TestCase):
    def test_original_and_decoded_payloads_execute_against_oracle(self):
        for family in ("chain", "diamond", "nested_cfg"):
            with self.subTest(family=family):
                original = workload(family, 4)
                original.code.verify()
                original.code.verify_type()
                decoded = basic.decode_json(basic.encode_json(original))
                decoded.code.verify()
                decoded.code.verify_type()
                self.assertTrue(original.code.is_structurally_equal(decoded.code))
                validate_shape(shape(original.code))
                validate_shape(shape(decoded.code))
                cases = validate_runtime(original, decoded, family, 4)
                self.assertEqual(len(cases), 7)
                if family == "nested_cfg":
                    self.assertEqual({case["path"] for case in cases}, {"a", "b", "c", "d"})
                    self.assertEqual(len({case["expected"] for case in cases[:4]}), 4)

    def test_wrong_arithmetic_in_real_payload_is_rejected(self):
        original = workload("diamond", 4)
        add = next(stmt for block in original.code.body.blocks for stmt in block.stmts if isinstance(stmt, Add))
        replacement = Sub(*add.args)
        replacement.result.type = add.result.type
        add.replace_by(replacement)
        original.code.verify()
        original.code.verify_type()
        decoded = basic.decode_json(basic.encode_json(original))
        with self.assertRaisesRegex(AssertionError, "oracle"):
            validate_runtime(original, decoded, "diamond", 4)

    def test_shape_differences_allowed_but_wrong_execution_rejected(self):
        cases = [dict(case, original=case["expected"], decoded=case["expected"])
                 for case in expected_cases("diamond", 4)]
        rust = {"family": "diamond", "size": 4, "semantic_cases": cases,
                "shape": "Rust representation", "operations": {"add": 2, "sub": 2}}
        python = dict(rust, shape="different Python representation", operations={"other": 7})
        assert_semantically_valid(rust, python, "diamond", 4)
        broken = copy.deepcopy(python)
        broken["semantic_cases"][0]["decoded"] += 1
        with self.assertRaisesRegex(AssertionError, "decoded runtime result"):
            assert_semantically_valid(rust, broken, "diamond", 4)

    def test_branch_arity_and_dominance_rejected_locally(self):
        invalid = shape(workload("diamond", 4).code)
        invalid[0]["ops"][-1]["inputs"].pop()
        with self.assertRaisesRegex(AssertionError, "arity"):
            validate_shape(invalid)
        invalid = shape(workload("diamond", 4).code)
        invalid[1]["ops"][0]["inputs"][0] = invalid[2]["arguments"][0]["id"]
        with self.assertRaisesRegex(AssertionError, "dominate"):
            validate_shape(invalid)


if __name__ == "__main__":
    unittest.main()
