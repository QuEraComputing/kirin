"""SSACFG region trait.

This module defines the SSACFGRegion trait, which is used to indicate that a
region has an SSACFG graph.
"""

from typing import TYPE_CHECKING
from dataclasses import dataclass

from kirin.ir.traits.abc import RegionTrait

if TYPE_CHECKING:
    from kirin.ir import Region


@dataclass(frozen=True)
class SSACFGRegion(RegionTrait):

    def get_graph(self, region: "Region"):
        from kirin.analysis.cfg import CFG

        return CFG(region)
