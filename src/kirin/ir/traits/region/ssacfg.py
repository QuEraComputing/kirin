"""SSACFG region trait.

This module defines the SSACFGRegion trait, which is used to indicate that a
region has an SSACFG graph.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, TypeVar
from dataclasses import dataclass

from kirin.ir.ssa import SSAValue as SSAValue
from kirin.ir.traits.abc import RegionGraph, RegionInterpretationTrait
from kirin.ir.nodes.region import Region

if TYPE_CHECKING:
    from kirin import ir
    from kirin.interp.frame import FrameABC


@dataclass(frozen=True)
class HasCFG(RegionGraph):

    def get_graph(self, region: ir.Region):
        from kirin.analysis.cfg import CFG

        return CFG(region)


@dataclass(frozen=True)
class SSACFG(RegionInterpretationTrait):

    ValueType = TypeVar("ValueType")

    @classmethod
    def set_region_input(
        cls, frame: FrameABC[SSAValue, ValueType], region: Region, *inputs: ValueType
    ) -> None:
        frame.set_values(region.blocks[0].args, inputs)
