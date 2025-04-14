from dataclasses import dataclass

from kirin import ir
from kirin.rewrite import abc, result
from kirin.dialects import py

from .stmts import X, Y, Z, Id, PauliOperator


@dataclass
class RewritePauliMult(abc.RewriteRule):
    def rewrite_Statement(self, node: ir.Statement) -> result.RewriteResult:
        if not isinstance(node, py.binop.Mult):
            return result.RewriteResult()

        if not isinstance(node.lhs.owner, PauliOperator) and not isinstance(
            node.rhs.owner, PauliOperator
        ):
            return result.RewriteResult()

        if isinstance(node.lhs.owner, py.Constant):
            assert isinstance(node.rhs.owner, PauliOperator)  # make the linter happy
            new_op = self.number_pauli_mult(node.lhs.owner, node.rhs.owner)
            node.replace_by(new_op)
            return result.RewriteResult(has_done_something=True)
        elif isinstance(node.rhs.owner, py.Constant):
            assert isinstance(node.lhs.owner, PauliOperator)  # make the linter happy
            new_op = self.number_pauli_mult(node.rhs.owner, node.lhs.owner)
            node.replace_by(new_op)
            return result.RewriteResult(has_done_something=True)

        if not isinstance(node.lhs.owner, PauliOperator) or not isinstance(
            node.rhs.owner, PauliOperator
        ):
            return result.RewriteResult()

        new_op = self.pauli_pauli_mult(node.lhs.owner, node.rhs.owner)
        node.replace_by(new_op)
        return result.RewriteResult(has_done_something=True)

    @staticmethod
    def number_pauli_mult(lhs: py.Constant, rhs: PauliOperator) -> PauliOperator:
        num = lhs.value.unwrap() * rhs.pre_factor
        return type(rhs)(pre_factor=num)

    @staticmethod
    def pauli_pauli_mult(lhs: PauliOperator, rhs: PauliOperator) -> PauliOperator:
        num = rhs.pre_factor * lhs.pre_factor

        if isinstance(lhs, type(rhs)):
            return Id(pre_factor=num)

        if isinstance(lhs, type(rhs)):
            return Id(pre_factor=num)

        if isinstance(lhs, Id):
            return type(rhs)(pre_factor=num)

        if isinstance(rhs, Id):
            return type(lhs)(pre_factor=num)

        if isinstance(lhs, X):
            if isinstance(rhs, Y):
                return Z(pre_factor=1j * num)
            elif isinstance(rhs, Z):
                return Y(pre_factor=-1j * num)

        if isinstance(lhs, Y):
            if isinstance(rhs, X):
                return Z(pre_factor=-1j * num)
            elif isinstance(rhs, Z):
                return X(pre_factor=1j * num)

        if isinstance(lhs, Z):
            if isinstance(rhs, Y):
                return X(pre_factor=-1j * num)
            elif isinstance(rhs, X):
                return Y(pre_factor=1j * num)

        raise RuntimeError("How on earth did we end up here?")
