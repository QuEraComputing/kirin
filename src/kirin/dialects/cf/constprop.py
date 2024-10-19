from kirin.analysis.dataflow.constprop import ConstProp, NotConst
from kirin.dialects.cf.dialect import dialect
from kirin.dialects.cf.stmts import Assert
from kirin.interp import ResultValue, impl

from .typeinfer import TypeInfer


@dialect.register(key="constprop")
class DialectConstProp(TypeInfer):

    @impl(Assert)
    def assert_stmt(self, interp: ConstProp, stmt: Assert, values):
        return ResultValue(NotConst())
