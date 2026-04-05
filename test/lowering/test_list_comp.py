from kirin import lowering
from kirin.prelude import basic_no_opt, structural_no_opt


def test_list_comp_lowers_with_cf():
    def main():
        return [x for x in range(3)]

    code = lowering.Python(basic_no_opt).python_function(main)

    assert code is not None


def test_list_comp_lowers_with_scf():
    def main():
        return [x for x in range(4) if x]

    code = lowering.Python(structural_no_opt).python_function(main)

    assert code is not None


def test_list_comp_nested_generators_lower():
    def main():
        return [(x, y) for x in range(2) for y in range(3) if y]

    code = lowering.Python(basic_no_opt).python_function(main)

    assert code is not None
