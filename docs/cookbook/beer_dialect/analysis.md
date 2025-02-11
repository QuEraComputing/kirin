## Beer price analysis

In this section we will discuss on how to perform analysis of a kirin program. We will again use our `beer` dialect example.

### Goal

Consider the following program:
```python
@beer
def main2(x: int):

    bud = NewBeer(brand="budlight")
    heineken = NewBeer(brand="heineken")

    bud_pints = Pour(bud, 12 + x)
    heineken_pints = Pour(heineken, 10 + x)

    Drink(bud_pints)
    Drink(heineken_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    Drink(bud_pints)
    Puke()

    return x
```

We would like to implement an forward dataflow analysis that walk through the program, and collect the price information of each statements.

### Define Lattice



### Custom Forward Data Flow Analysis
