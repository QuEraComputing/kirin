from kirin.analysis.callgraph import CallGraph as CallGraph
from kirin.analysis.cfg import CFG as CFG
from kirin.analysis.dataflow.constprop import ConstProp as ConstProp
from kirin.analysis.dataflow.forward import (
    Forward as Forward,
    ForwardExtra as ForwardExtra,
)
from kirin.analysis.dataflow.lattice import const as const, purity as purity
from kirin.analysis.dataflow.lattice.infer import InferenceLattice as InferenceLattice
from kirin.analysis.dataflow.typeinfer import TypeInference as TypeInference
