from kirin.prelude import basic


def test_fold_pass():
    @basic(fold=True)
    def _for_loop_append_xtones(
        cntr: int,
        x: tuple,
        n_range: int,
        channel_group_x: str,
    ):
        if cntr < n_range:
            x = x + (channel_group_x,)
            return _for_loop_append_xtones(cntr + 1, x, n_range, channel_group_x)
        else:
            return x

    @basic(fold=True)
    def my_lambd(ch_group_x: str):
        def my_func(x: int):
            return _for_loop_append_xtones(0, (), x, ch_group_x)

        return my_func

    @basic(fold=True)
    def my_call():
        x = my_lambd("x")
        return x(3)

    my_call.code.print()
    assert len(my_call.callable_region.blocks[0].stmts) == 2
    x = my_call()
    assert x == ("x", "x", "x")


test_fold_pass()
