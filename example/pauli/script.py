from .group import pauli
from .stmts import X, Y, Z


@pauli
def main():
    ex = (X() + 2 * Y()) * Z()
    return ex


main.print()
