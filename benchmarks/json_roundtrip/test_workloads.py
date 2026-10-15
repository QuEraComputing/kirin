"""Run with the pinned baseline Python: -m unittest discover -s this_directory."""
import copy
import unittest

from kirin.prelude import basic
from python_benchmark import describe, shape, workload
from validation import assert_equivalent, validate_shape


class WorkloadValidity(unittest.TestCase):
    def test_all_families_valid_before_and_after_json(self):
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
                self.assertEqual(shape(original.code), shape(decoded.code))

    def test_shape_comparison_rejects_operand_target_and_type_changes(self):
        original = workload("diamond", 4)
        operations, blocks = describe(original.code)
        reference = {"family": "diamond", "adds": 4, "operations": operations,
                     "blocks": blocks, "shape": shape(original.code)}
        for mutation in ("operand", "target", "type"):
            changed = copy.deepcopy(reference)
            if mutation == "operand":
                changed["shape"][1]["ops"][0]["inputs"][1] = 2
            elif mutation == "target":
                changed["shape"][0]["ops"][-1]["targets"][0] = 2
            else:
                changed["shape"][0]["arguments"][1]["type"] = "float"
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                assert_equivalent(reference, changed)

    def test_branch_arity_and_dominance_rejected(self):
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
