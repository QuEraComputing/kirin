# Pauli Matrix Algebra

In this example, we'll implement a simple dialect that allows you to write mathematical expressions using the [Pauli matrices](https://en.wikipedia.org/wiki/Pauli_matrices) $\sigma_x, \sigma_y$ and $\sigma_z$.
We'll add some basic rewriting routines so that the IR is symbolically rewritten using the basic relations

$$
\sigma_i \sigma_j = \epsilon_{ijk} \sigma_k,
$$

and

$$
\sigma_i^2 = \mathbb{I_2}
$$

Given these relations, you can rewrite longer expressions at compile time, such that the resulting code that is executed becomes much simpler and the number of matrix operations that need to be performed is reduced.

We'll start out with the basic definitions and then add more features to it as we go along.


## Defining the dialect

First off, we'll define the dialect statements.
Each statement defining a Pauli operator will be returning a numpy array.
We can keep things concise by defining the properties in one class and then inheriting from that class for each statement.

Each statement that you want to use in your dialect needs to be registered to it.
That's done by providing your dialect as an argument to the [`statement`][kirin.decl.statement].

```python
from numbers import Number
import numpy as np

from kirin import ir, lowering
from kirin.types import PyClass
from kirin.decl import statement, info


dialect = ir.Dialect("pauli")  # (1)!


@statement  # (2)!
class PauliOperator(ir.Statement):
    traits = frozenset({ir.Pure(), lowering.FromPythonCall()})  # (3)!
    pre_factor: Number = info.attribute(default=1)  # (4)!
    result: ir.ResultValue = info.result(PyClass(np.matrix))


@statement(dialect=dialect)  # (5)!
class X(PauliOperator):
    pass

@statement(dialect=dialect)
class Y(PauliOperator):
    pass

@statement(dialect=dialect)
class Z(PauliOperator):
    pass


@statement(dialect=dialect)
class Id(PauliOperator):
    pass
```


1. Define the actual dialect to which we want to register our statements.
2. Notice how we are not registering the `PauliOperator`. That's because we don't actually want users to create an instant of it since it serves just as a base type. This is similar to how a user cannot create a number of base type `Number`, it's always a type such as `float` or `int`.
3. All Pauli operator statements are pure, since they have no side-effects and their output is fully determined by the input. They are also created by a simple call.
4. In addition to the actual matrix, our Pauli operator statements will also have a pre-factor that's just a number. This makes rewriting easier later, read on for more details.
5. Concrete statements have to be registered to our dialect.


With these few simple statements defined, we can already create a basic decorator that does no rewrites or optimizations, but at least knows about our statements.
Note that we are adding our dialect to the [`basic_no_opt`][kirin.prelude.basic_no_opt] dialect group.
This defines some basic semantics such as multiplication.
In practice, you'd very rarely ever want to start from scratch.
There are a number of different dialect groups pre-defined in kirin, from which to start.


```python
from kirin.prelude import basic_no_opt


@ir.dialect_group(basic_no_opt.add(dialect=dialect))
def pauli_basic_no_opt(self):
    def run_pass(mt):
        pass  # (1)!

    return run_pass
```

1. As a start, our dialect will do nothing with the IR.

With that, we can use our DSL within a function that is decorated accordingly.

```python
@pauli_basic_no_opt
def basic_example():
    x = X()
    y = Y()
    z = x * y
    return z

basic_example.print()
```

The printed IR shows that the statements are executed fine, but nothing more is happening.

```python
func.func basic_example() -> !Any {
  ^0(%basic_example_self):
  │ %x = pauli.x(){pre_factor=1 : !py.Number} : !py.matrix
  │ %y = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %z = py.binop.mult(%x, %y) : ~T
  │      func.return %z
} // func.func basic_example
```

Ideally, we'd like the IR to be rewritten, such that the result will just be

$$
\sigma_x \sigma_y = i \sigma_z
$$

In the following, we'll add a corresponding rewrite pass to our Pauli DSL.


## Rewriting multiplications

In order to rewrite the IR of our DSL, we'll need to define a [`RewriteRule`][kirin.rewrite.abc.RewriteRule] that does what we want and then add it to the DSL as a pass.
A [`RewriteRule`][kirin.rewrite.abc.RewriteRule] takes node of our IR expression as an argument and then rewrites it (in-place) according to any rule that you specify.

Since we want to rewrite statements of our dialect, the entry point we define in our rule will be the `rewrite_Statement` method.


```python
from dataclasses import dataclass

from kirin.rewrite import abc, result
from kirin.dialects import py


@dataclass
class RewritePauliMult(abc.RewriteRule):
    def rewrite_Statement(self, node: ir.Statement) -> result.RewriteResult:
        if not isinstance(node, py.binop.Mult):  # (1)!
            return result.RewriteResult()

        if not isinstance(node.lhs.owner, PauliOperator) and not isinstance(node.rhs.owner, PauliOperator):  # (2)!
            return result.RewriteResult()

        if isinstance(node.lhs.owner, py.Constant): # (3)!
            new_op = self.number_pauli_mult(node.lhs.owner, node.rhs.owner)
            node.replace_by(new_op)
            return result.RewriteResult(has_done_something=True)
        elif isinstance(node.rhs.owner, py.Constant): # (4)!
            new_op = self.number_pauli_mult(node.rhs.owner, node.lhs.owner)
            node.replace_by(new_op)
            return result.RewriteResult(has_done_something=True)


        if not isinstance(node.lhs.owner, PauliOperator) or not isinstance(node.rhs.owner, PauliOperator):  # (5)!
                return result.RewriteResult()

        new_op = self.pauli_pauli_mult(node.lhs.owner, node.rhs.owner)  #(6)!
        node.replace_by(new_op)
        return result.RewriteResult(has_done_something=True)

    ...
```

1. Make sure we're looking at a multiplication.
2. If not at least one of the constituents of the is a subtype of `PauliOperator` we don't want to do anything. This also makes sure the `Statement`s in our multiplication are owned by a `Statement` (see below for more details).
3. This is a multiplcation with a number on the left-hand-side.
4. This is a multiplcation with a number on the right-hand-side. We can just call the same method to rewrite this, but swap the sides. That's fine as things commute.
5. None of the consituents is a number, one of them is a Pauli matrix. With this check we make sure that both of them are Pauli matrices, otherwise we bail (we could also be looking at a multiplication of e.g. additions).
6. Now, we know for sure we have two Pauli matrices that need to be rewritten according to their commutation relation.

Note how we're always checking the type by looking at the `.owner` of the `lhs` and `rhs` of the multiplcation. This achieves two things:
first, it allows to actually determine the type of each of the nodes for basic checks (such as making sure that at least one of the arguments to the multiplication is a Pauli matrix).
We decide how to rewrite the expression based on this.
Secondly, this also makes sure that the owner is a `Statement` rather than a [`Block`][kirin.ir.Block].
That means we're actually looking at a statement rather than an argument to a block (think, a function argument) and we're fine doing a rewrite.
If the check is `False`, i.e. if we are looking at a [`Block`][kirin.ir.Block]-owned Pauli matrix or a multiplication that doesn't contain any Pauli matrices, we simply bail and don't do a rewrite.

So, the above `rewrite_Statement` method just runs a few basic checks and forwards to an appropriate rewriting method depending on the argument types of the multiplication.

Now, let's have a look at the actual rewriting methods.

```python
    @staticmethod
    def number_pauli_mult(lhs: py.Constant, rhs: PauliOperator) -> PauliOperator:
        num = lhs.value.unwrap() * rhs.pre_factor
        return type(rhs)(pre_factor=num)  # (1)!

    @staticmethod
    def pauli_pauli_mult(lhs: PauliOperator, rhs: PauliOperator) -> PauliOperator:
        num = rhs.pre_factor * lhs.pre_factor

        if isinstance(lhs, type(rhs)):  # (2)!
            return Id(pre_factor=num)

        if isinstance(lhs, type(rhs)):
            return Id(pre_factor=num)

        if isinstance(lhs, Id):  # (3)!
            return type(rhs)(pre_factor=num)

        if isinstance(rhs, Id):
            return type(lhs)(pre_factor=num)

        if isinstance(lhs, X):  # (4)
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

        raise RuntimeError("How on earth did we end up here?")  # (5)!

```

1. When rewriting a multiplication with a number, we just construct the same Pauli matrix with an updated pre-factor.
2. If both Pauli matrices are of the same type, the result will be an identity matrix.
3. If either one is an identity, we just return the other argument with an updated pre-factor.
4. For all other cases, we have to explicitly check both types and return the third one according to the relation shown at the beginning of this example.
5. The cases above should be exhaustive, so if we reach this point, there's a bug in our logic.


The method rewriting a multiplication with a number is rather straightforward.
We simply return the same Pauli matrix, but with an updated pre-factor.
To obtain the constructor from the expression we simply use e.g. `type(rhs)`.

The second method, which rewrites products of Pauli matrices, is a bit more interesting.
We have to cover a number of cases for the different types we can encounter and rewrite things accordingly.

Finally, let's put our rewriter to the test.
We create another decorator for our dialect, adding in a rewrite pass that walks through our expression tree and rewrites according to the rule defined above.

```python
from kirin.rewrite import Walk


@ir.dialect_group(basic_no_opt.add(dialect=dialect))
def pauli_mul_opt(self):
    def run_pass(mt):
        Walk(RewritePauliMult()).rewrite(mt.code)

    return run_pass


@pauli_mul_opt
def advanced_example():
    x = 2*X()
    y = Y()
    z = x * y
    return z


advanced_example.print()
```

Success!
Looking at the IR that is printed out by the last line,

```python
func.func advanced_example() -> !Any {
  ^0(%advanced_example_self):
  │ %0 = py.constant.constant 2 : !py.int
  │ %1 = pauli.x(){pre_factor=1 : !py.Number} : !py.matrix
  │ %x = pauli.x(){pre_factor=2 : !py.Number} : !py.matrix
  │ %y = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %z = pauli.z(){pre_factor=2j : !py.Number} : !py.matrix
  │      func.return %z
} // func.func advanced_example
```

we see that the return value assigned in the second-to-last line, `%z`, is rewritten as an instance of a `Z` operator statement of our dialect.
Actually, since the rewriting pass is now smart enough to figure out how to rewrite the expressions, the rest of SSA values that are assigned are no longer needed.

We can use a [`Fold`][kirin.passes.Fold] in order to get rid of those.

```python
from kirin.passes import Fold


@ir.dialect_group(basic_no_opt.add(dialect=dialect))
def pauli_mul_opt_fold(self):
    fold_pass = Fold(self)

    def run_pass(mt):
        Walk(RewritePauliMult()).rewrite(mt.code)
        fold_pass(mt)  # (1)!

    return run_pass


@pauli_mul_opt_fold
def advanced_example_fold():
    x = 2*X()
    y = Y()
    z = x * y
    return z


advanced_example_fold.print()
```

1. Make sure to run the folding pass *after* the rewrite.

The resulting IR is

```python
func.func advanced_example_fold() -> !Any {
  ^0(%advanced_example_fold_self):
  │ %z = pauli.z(){pre_factor=2j : !py.Number} : !py.matrix
  │      func.return %z
} // func.func advanced_example_fold
```

Fantastic!
Our entire function now got rewritten to a single statement.
Just adding the pre-defined [`Fold`][kirin.passes.Fold] pass gave us constant folding for free.
This nicely illustrates how powerful the composability of passes is in kirin.

Even better, since the IR is first written in SSA form and all multiplications are only binary operators, our rewriter already works for arbitrarily long chains of multiplications.

```python
@pauli_mul_opt_fold
def nested_multiplication_example():
    return 7 * X() * Y() * Y() * Z() * X() * Z()


nested_multiplication_example.print()
```

The resulting IR is again a function that simply instantiates a single Pauli matrix:

```python
func.func nested_multiplication_example() -> !Any {
  ^0(%nested_multiplication_example_self):
  │ %0 = pauli.id(){pre_factor=(-7+0j) : !py.Number} : !py.matrix
  │      func.return %0
} // func.func nested_multiplication_example
```

To see how the IR is first written in SSA form, assigning values line-by-line for intermediary steps, turn off folding by using the `@pauli_mul_opt` decorator for the function instead.

Okay, so this is pretty cool, but the code we used so far isn't very involved.
Now, let's see what happens if use an addition inside a multiplcation.

```python
@pauli_mul_opt_fold
def addition_example():
    z = (X() + Y()) * Y()
    return z

addition_example.print()
```

Here's the resulting IR:

```python
func.func addition_example() -> !Any {
  ^0(%addition_example_self):
  │ %0 = pauli.x(){pre_factor=1 : !py.Number} : !py.matrix
  │ %1 = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %2 = py.binop.add(%0, %1) : ~T
  │ %3 = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %z = py.binop.mult(%2, %3) : ~T
  │      func.return %z
} // func.func addition_example
```

Oh no!
It seems our rewriting rule did nothing here.
That's because it doesn't know how to deal with a multiplication containing an addition, which is the value assigned to `%z` in the IR.
Subsequently, there are also no constants to be folded in the end.
We'll see how to extend our Pauli DSL in order to deal with this.


## Chaining rewriters

We could deal with the issue we're facing by adding another `if` branch in our `RewritePauliMult.rewrite_Statement` method above.
However, we might later want to enhance it further and at some point the implementation will get quite messy.

Also, we can take a step back and think about what we actually want to have here:
Basically, all we'd need is to rewrite the addition using distribution, i.e. $(a + b) c = ac + bc$.
Then, the multiplication rewriter we already have could work it's magic on the multiplication instances.
This rewriting of a multiplication containing additions has nothing to do with Pauli matrices, though.
It's something more general, so it makes sense to separate this rewriting logic.

In this section, we'll implement another rewriting rule that distributes multiplications containing additions and then add this to our dialect such that both rewriting rules are applied.

We start out by defining the rewriter.
The basic principle is the same as before.

```python
@dataclass
class RewriteDistributeMult(abc.RewriteRule):
    def rewrite_Statement(self, node: ir.Statement) -> result.RewriteResult:
        if not isinstance(node, py.binop.Mult):  # (1)!
            return result.RewriteResult()

        if isinstance(node.lhs.owner, py.binop.Add):  # (2)!
            m1 = py.binop.Mult(node.lhs.owner.lhs, node.rhs)  # (3)!
            m2 = py.binop.Mult(node.lhs.owner.rhs, node.rhs)

            m1.insert_before(node)  # (4)!
            m2.insert_before(node)

            a = py.binop.Add(m1.result, m2.result)  # (5)!
            node.replace_by(a)  # (6)!
            return result.RewriteResult(has_done_something=True)

        if isinstance(node.rhs.owner, py.binop.Add):
            m1 = py.binop.Mult(node.lhs, node.rhs.owner.lhs)
            m2 = py.binop.Mult(node.lhs, node.rhs.owner.rhs)

            m1.insert_before(node)
            m2.insert_before(node)

            a = py.binop.Add(m1.result, m2.result)
            node.replace_by(a)
            return result.RewriteResult(has_done_something=True)

        return result.RewriteResult()
```

1. Again, we want to make sure we're looking at a multiplication.
2. The left-hand-side argument of the multiplication is an addition in this case. Let's rewrite it.
3. The addition is binary, so we define to multiplication instances that consist of the appropriate arguments. Note that the order here matters, since e.g. Pauli matrices don't commute.
4. In order to replace our node by an addition of multiplications, we need to make sure the newly created multiplication statements are actually part of the current block. Also, they need to be injected before the node.
5. Define the addition containing the multiplications.
6. Replace the current node by the addition defined above.

The rewriter just looks at two cases, where either one of the arguments to the multiplication is an addition.
Now, let's define a DSL that uses both rewriting passes.
To compose them, we'll use a [`Chain`][kirin.rewrite.Chain] to create a rewriter that runs both rewrite passes.

```python
from kirin.rewrite import Chain


@ir.dialect_group(basic_no_opt.add(dialect=dialect))
def pauli_add(self):
    fold_pass = Fold(self)

    def run_pass(mt):
        Walk(
            Chain(RewriteDistributeMult(), RewritePauliMult())
            ).rewrite(mt.code)
        fold_pass(mt)

    return run_pass
```


Great, let's try it out!

```python
@pauli_add
def addition_that_kinda_works():
    z = (X() + Y()) * Y()
    return z


addition_that_kinda_works.print()
```

From the resulting IR, we see that this kind of works:

```python
func.func addition_that_kinda_works() -> !Any {
  ^0(%addition_that_kinda_works_self):
  │ %0 = pauli.x(){pre_factor=1 : !py.Number} : !py.matrix
  │ %1 = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %2 = pauli.y(){pre_factor=1 : !py.Number} : !py.matrix
  │ %3 = py.binop.mult(%0, %2) : ~T
  │ %4 = py.binop.mult(%1, %2) : ~T
  │ %z = py.binop.add(%3, %4) : ~T
  │      func.return %z
} // func.func addition_that_kinda_works
```

The return value `%z` now is an addition containing the multiplication.
So at least the rewriter we just implemented seems to work.
However, the multiplication instances of Pauli matrices are not rewritten.

The reason for this is that the rewrite pass tries to rewrite the multiplications first, which still contains the addition at the time.
What we really want, however, is to run both passes over and over until no further simplifications are possible.
This can be achieved by using a [`Fixpoint`][kirin.rewrite.Fixpoint], which does exactly that.

With that, let's finally define our complete `pauli` DSL.

```python
from kirin.rewrite import Fixpoint


@ir.dialect_group(basic_no_opt.add(dialect=dialect))
def pauli(self):
    fold_pass = Fold(self)

    def run_pass(mt):
        Fixpoint(Walk(
            Chain(RewriteDistributeMult(), RewritePauliMult())
            )).rewrite(mt.code)
        fold_pass(mt)

    return run_pass
```

Note, that we need to make sure to wrap the [`Walk`][kirin.rewrite.Walk] by the [`Fixpoint`][kirin.rewrite.Fixpoint], since we are changing the structure of the IR and thus need to walk through the whole thing over and over.

Now, let's try this out.

```python
@pauli
def addition_that_works():
    z = (X() + Y()) * Y()
    return z


addition_that_works.print()
```

And here's the result:

```python
func.func addition_that_works() -> !Any {
  ^0(%addition_that_works_self):
  │ %0 = pauli.z(){pre_factor=1j : !py.Number} : !py.matrix
  │ %1 = pauli.id(){pre_factor=1 : !py.Number} : !py.matrix
  │ %z = py.binop.add(%0, %1) : ~T
  │      func.return %z
} // func.func addition_that_works
```

Great, both rewriters now do their job as we'd expect.
The resulting return value is an addition of just two Pauli matrices, each of which is the result of a rewritten multiplication.

Using this, we can be sure that the actual number of matrix multiplications we need to perform in the end will be minimal, since they are rewritten at compile time.

```python
@pauli
def cool_example():
    x = X()
    y = Y()
    z = Z()
    ex = 2*(x + y) * (z + y)
    return ex


cool_example.print()
```

As you can see from the IR,

```python
func.func cool_example() -> !Any {
  ^0(%cool_example_self):
  │  %0 = pauli.y(){pre_factor=-2j : !py.Number} : !py.matrix
  │  %1 = pauli.z(){pre_factor=2j : !py.Number} : !py.matrix
  │  %2 = py.binop.add(%0, %1) : ~T
  │  %3 = pauli.x(){pre_factor=2j : !py.Number} : !py.matrix
  │  %4 = pauli.id(){pre_factor=2 : !py.Number} : !py.matrix
  │  %5 = py.binop.add(%3, %4) : ~T
  │ %ex = py.binop.add(%2, %5) : ~T
  │       func.return %ex
} // func.func cool_example
```

the result here is just a number of additions containing other additions and Pauli matrices.
That's it!
Next, we'll see how we can actually run the code we're writing with our DSL.


## Method implementation

Before we can execute the code that is represented by the IR, we'll need to register methods for each statement to our dialect.
This is done by defining a [`MethodTable`][kirin.interp.MethodTable] and registering it to the dialect.
Then, the interpreter knows how to deal with statements such as `X()`.

As the typing of the `result` field suggests, we want each statement to return a `np.matrix`.

```python
import numpy as np

from kirin.interp import MethodTable, impl


@dialect.register
class PauliMethods(MethodTable):
    X_mat = np.array([[0, 1], [1, 0]])
    Y_mat = np.array([[0, -1j], [1j, 0]])
    Z_mat = np.array([[1, 0], [0, -1]])
    Id_mat = np.array([[1, 0], [0, 1]])

    @impl(X)  # (1)!
    def x(self, interp, frame, stmt: X):
        return (stmt.pre_factor * self.X_mat, )

    @impl(Y)
    def y(self, interp, frame, stmt: Y):
        return (self.Y_mat * stmt.pre_factor, )

    @impl(Z)
    def z(self, interp, frame, stmt: Z):
        return (self.Z_mat * stmt.pre_factor, )

    @impl(Id)
    def id(self, interp, frame, stmt: Id):
        return (self.Id_mat * stmt.pre_factor, )



print(cool_example())  # (2)!
```

1. Register an implementation for the statement `X()`.
2. Notice that we are actually *calling* the function this time.

And, sure enough, when running the code, we now obtain a $2x2$ matrix:

```python
[[ 2.+2.j -2.+2.j]
 [ 2.+2.j  2.-2.j]]
```

Also, now that the method table is registered to the dialect, constant folding can really work its magic.
To see that, let's define another function and look at its IR.
Note, that it wouldn't work the same for any function defined so far, since they were defined *before* the methods were registered to the dialect.

```python
@pauli
def the_coolest_example():
    ex = (X() + 2*Y()) * Z()
    return ex

the_coolest_example.print()
```

And now, check out the IR:

```python
func.func the_coolest_example() -> !Any {
  ^0(%the_coolest_example_self):
  │ %ex = py.constant.constant array([[ 0.+0.j, -1.+2.j],
       [ 1.+2.j,  0.+0.j]]) : !py.ndarray
  │       func.return %ex
} // func.func the_coolest_example
```

Well, look at this.
No additions or multiplications left, the function is just returning a single matrix.
This is because the constant folding pass uses the fact that all of the statements are pure.
Then, it invokes the actual methods and resolves additions of matrices.
So everything is evaluated at compile time.
That's neat!
And we got all of that basically for free, just by providing methods.
Amazing.

But, wait:
if constant folding can evaluate additions, couldn't it also evaluate multiplications?
Wouldn't the make our multiplication rewrite pass obsolete?
Sort of:
this would mean that you still evaluate a whole bunch of matrix products, just at compile time rather than at runtime.
Our rewriting pass, instead uses a symbolic approach to rewrite multiplications without the need to actually evaluate a matrix multiplication.
Of course, since we are talking about 2x2 matrices only, the difference won't be very large.
However, this is just an example to illustrate how to do something like this.
