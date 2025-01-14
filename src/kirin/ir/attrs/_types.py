from dataclasses import dataclass

from .abc import Attribute


@dataclass
class _TypeAttribute(Attribute):
    pass
