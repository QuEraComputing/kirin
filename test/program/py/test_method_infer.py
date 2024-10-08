from kirin.prelude import basic

# print(hash())
# print(hash((types.Int, types.Float)))


@basic(typeinfer=True)
def foo(x: int):
    if x > 1:
        return x + 1
    else:
        return x - 1.0


# infer = TypeInference(basic)
# infer.eval(foo, foo.arg_types).expect().print()
foo.code.print()


@basic(typeinfer=True)
def main(x: int):
    return foo(x)


main.code.print()
