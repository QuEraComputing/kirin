import numpy as np

from kirin.interp import MethodTable, impl

from .stmts import X, Y, Z, Id
from .dialect import _dialect


@_dialect.register
class PauliMethods(MethodTable):
    X_mat = np.array([[0, 1], [1, 0]])
    Y_mat = np.array([[0, -1j], [1j, 0]])
    Z_mat = np.array([[1, 0], [0, -1]])
    Id_mat = np.array([[1, 0], [0, 1]])

    @impl(X)  # (1)!
    def x(self, interp, frame, stmt: X):
        return (stmt.pre_factor * self.X_mat,)

    @impl(Y)
    def y(self, interp, frame, stmt: Y):
        return (self.Y_mat * stmt.pre_factor,)

    @impl(Z)
    def z(self, interp, frame, stmt: Z):
        return (self.Z_mat * stmt.pre_factor,)

    @impl(Id)
    def id(self, interp, frame, stmt: Id):
        return (self.Id_mat * stmt.pre_factor,)
