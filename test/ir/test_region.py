from kirin.prelude import basic_no_opt


@basic_no_opt
def factorial(n):
    if n == 0:
        return 1
    else:
        return n * factorial(n - 1)


def test_region_clone():
    assert factorial.callable_region.clone().is_structurally_equal(
        factorial.callable_region
    )
