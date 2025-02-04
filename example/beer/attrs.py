from dataclasses import dataclass


# runtime value of a beer
@dataclass
class Beer:
    brand: str


@dataclass
class Pints:
    kind: Beer
    amount: int
