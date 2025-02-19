from dataclasses import dataclass


# runtime value of a food
@dataclass
class Food:
    type: str


@dataclass
class Serving:
    kind: Food
    amount: int
