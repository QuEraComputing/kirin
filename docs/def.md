!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Understanding Kirin IR Declarations

In this section, we will learn about the terminology used in Kirin IR. This will help you understand the structure of the IR and how to write your own compiler using Kirin.

!!! note
    The examples in this section will also contain the equivalent MLIR and xDSL code to help you understand the differences between them if you are already familiar with MLIR or xDSL.

## Dialect

The [`Dialect`][kirin.ir.Dialect] object is the main registry of all the statements and attributes that are available in the IR. You can create a dialect by just following:

```python
from kirin import ir
dialect = ir.Dialect("my_dialect") # (1)!
```

1. The [`Dialect`][kirin.ir.Dialect] object is created with the name `my_dialect`.

## Dialect Groups

A dialect group is a collection of dialects that can be used as a decorator for Python frontend. It is used to group multiple dialects together and define the passes, compiler options, and other configurations for the dialects.

```python
from kirin.ir import Method, dialect_group

@dialect_group(
    [
        base,
        binop,
        cmp,
        unary,
        assign,
        attr,
        boolop,
        constant,
        indexing,
        func,
    ]
) # (1)!
def python_basic(self): # (2)!
    def run_pass(mt: Method) -> None: # (3)!
        pass # (4)!
    return run_pass
```

1. The [`dialect_group`][kirin.ir.dialect_group] decorator is used to create a dialect group with the specified dialects. In this case, we construct a basic Python dialect that allows some basic operations.
2. The `python_basic` function is the entry point of the dialect group. It takes a `self` argument, which is the [`DialectGroup`][kirin.ir.group.DialectGroup] object. This argument is used to access the definition of the dialect group and optionally update the dialect group.
3. The `run_pass` function is the function that will be called when the dialect group is applied to a given Python function. This is where you can define the passes that will be applied to the method. See the next example.

!!! note
    Unlike MLIR/LLVM, because Kirin focuses on kernel functions, the minimal unit of compilation is a function. Therefore, the compiler pass always passes a [`ir.Method`][kirin.ir.Method] object which contains a function-like statement (a statement has [`ir.traits.CallableStmtInterface`][kirin.ir.traits.CallableStmtInterface]).

The above dialect group `python_basic` allows you to use it as following:

```python
@python_basic
def my_function():
    pass
```

However, if we want to run some compilation passes on the function, we need to define some passes in the `run_pass` function.

```python
from kirin.passes.fold import Fold

@dialect_group(python_basic) # (1)!
def python(self):
    fold_pass = Fold(self) # (2)!

    def run_pass(mt: Method, *, verify: bool = True, fold: bool = True) -> None: # (3)!
        if verify: # (4)!
            mt.verify()

        if fold: # (5)!
            fold_pass(mt)
    return run_pass
```

1. The [`dialect_group`][kirin.ir.dialect_group] decorator can also take a dialect group as an argument. This will use the dialects defined in the given dialect group with different passes.
2. The `Fold` pass is created when initializing the dialect group. This pass is used later when running the `run_pass` function.
3. The `run_pass` function is the function that will be called when the dialect group is applied to a given Python function. This function takes a `mt` argument, which is the [`ir.Method`][kirin.ir.Method] object, and optional arguments `verify`, `fold`, and `aggressive`.
4. If the `verify` argument is `True`, the method will be verified.
5. If the `fold` argument is `True`, the `Fold` pass will be applied to the method.

The above dialect group `python` allows you to use it as following:

```python
@python(fold=True) # (1)!
def my_function():
    pass
```

1. The `fold` argument here is passed to the `run_pass` function defined in the dialect group. Looks complicated? Don't worry, the `@dialect_group` decorator will handle everything including the type hints!

## Statement

In Kirin IR, a statement describes an operation that can be executed. Statements are the building blocks that contain the semantics of the program.

### Defining a Statement

While a statement can be hand-written by inheriting [`ir.Statement`][kirin.ir.Statement],
we provide a python-`dataclass`-like decorator [`statement`][kirin.decl.statement] and in combine
with the [`info.argument`][kirin.decl.info.argument],[`info.result`][kirin.decl.info.result],[`info.region`][kirin.decl.info.region], [`info.block`][kirin.decl.info.block] field specifier to make it easier to define a statement.

=== "Kirin"

    ```python
    from kirin import ir
    from kirin.decl import statement, info

    @statement # (1)!
    class MyStatement(ir.Statement): # (2)!
        name = "awesome" # (3)!
        traits = frozenset({ir.Pure()}) # (4)!
        # blabla, we will talk about this later
    ```

    1. the decorator [`@statement`][kirin.decl.statement] is used to generate implementations for the `MyStatement` class based on the fields defined in the class.
    2. The `MyStatement` class inherits from [`ir.Statement`][kirin.ir.Statement].
    3. The `name` field is the name of the statement, if your desired name is just `my_statement`, you can omit this field, [`@statement`][kirin.decl.statement] will automatically generate the name by converting the class name to snake case. The name is what will be used in text/pretty printing.
    4. The `traits` field is used to specify the traits of the statement. In this case, the statement is pure.

=== "MLIR"

    ```mlir
    class MyOp<string mnemonic, list<Trait> traits = []> :
        Op<MyDialect, mnemonic, traits>;

    def dialect_MyOp : MyOp<"awesome"> {
        let summary = "my awesome statement";

        let description = [{my awesome statement}];

        let arguments = (
            // some inputs, e.g ins F64ElementsAttr:$value from the MLIR Toy example
        );

        let results = (
            // some outputs, e.g outs F64Tensor from the MLIR Toy example
        );
      }
    ```

=== "xDSL"

    ```python
    from xdsl.irdl import IRDLOperation, irdl_op_definition, traits_def

    @irdl_op_definition
    class MyStatement(IRDLOperation):
        name = "awesome"
        traits = traits_def(Pure())
    ```

Like a function, a statement can have multiple inputs and outputs.

=== "Kirin"

    ```python
    @statement # (1)!
    class Add(ir.Statement):
        traits = frozenset({ir.Pure()}) # (2)!
        lhs: ir.SSAValue = info.argument(ir.types.Int) # (3)!
        rhs: ir.SSAValue = info.argument(ir.types.Int) # (4)!
        output: ir.ResultValue = info.result(ir.types.Int) # (5)!
    ```

    1. the decorator [`@statement`][kirin.decl.statement] is used to generate implementations for the `MyStatement` class based on the fields defined in the class.
    2. The `traits` field is used to specify the traits of the statement. In this case, the statement is pure.
    3. The `lhs` field is the left-hand side input value of the statement. The field descriptor [`info.argument`][kirin.decl.info.argument] is used to specify the type of the input value.
    4. The `rhs` field is the right-hand side input value of the statement. The field descriptor [`info.argument`][kirin.decl.info.argument] is used to specify the type of the input value.
    5. The `output` field is the output value of the statement. The field descriptor [`info.result`][kirin.decl.info.result] is used to specify the type of the output value.

=== "MLIR"

    ```mlir
    def dialect_Add : MyOp<"add"> {
        let summary = "my add statement";

        let description = [{my add statement}];

        let arguments = (
            AnySignlessInteger:$lhs,
            AnySignlessInteger:$rhs
        );

        let results = (outs I64:$output);
    }
    ```

=== "xDSL"

    ```python
    from xdsl.irdl import IRDLOperation, irdl_op_definition, traits_def

    @irdl_op_definition
    class Add(IRDLOperation):
        T: ClassVar = VarConstraint("T", signlessIntegerLike)
        lhs = operand_def(T)
        rhs = operand_def(T)
        result = result_def(T)
        assembly_format = "$lhs `,` $rhs attr-dict `:` type($result)"
    ```

A statement can have blocks as successors, which describe the control flow of the program.

!!! note
    For Kirin, because `Statement`s are just dataclass, the `__init__` method is generated
    in the same convention as Python `dataclass`. The field descriptor `info.xxx` are also using
    standard `dataclass` field descriptor convention. Most of the time, one can just use the default dataclass `__init__`. For comparison, in MLIR, the `BranchOp` will need a custom builder to add the operands and successors. Similarly, in xDSL, the `BranchOp` will need a `__init__` method to add the operands and successors.

=== "Kirin"

    ```python
    @statement
    class Branch(Statement):
       name = "br"
       traits = frozenset({IsTerminator()}) # (1)!

       arguments: tuple[SSAValue, ...] # (2)!
       successor: Block = info.block() # (3)!
    ```

    1. The `traits` field is used to specify the traits of the statement. In this case, the statement is a [terminator](/101.md/#terminator).
    2. The `arguments` field is the input values of the statement. Branch can take multiple arguments, `tuple[SSAValue, ...]` is used to specify that the field is a tuple of `SSAValue`. Note that only `...` is supported because if the number of arguments is known, we recommend specifying them explicitly.
    3. The `successor` field is the block that the statement will go to after execution. The field descriptor [`info.block`][kirin.decl.info.block] is used to specify the type of the field.

=== "MLIR"

    ```mlir
    def BranchOp : CF_Op<"br", [
       DeclareOpInterfaceMethods<BranchOpInterface, ["getSuccessorForOperands"]>,
       Pure, Terminator
     ]> {
        let arguments = (ins Variadic<AnyType>:$destOperands);
        let successors = (successor AnySuccessor:$dest);

        let builders = [
            OpBuilder<(ins "Block *":$dest,
                CArg<"ValueRange", "{}">:$destOperands), [{
            $_state.addSuccessors(dest);
            $_state.addOperands(destOperands);
            }]>];
    }
    ```

=== "xDSL"

    ```python
    from xdsl.irdl import IRDLOperation, irdl_op_definition, traits_def

    @irdl_op_definition
    class BranchOp(IRDLOperation):
        """Branch operation"""

        name = "cf.br"

        arguments = var_operand_def()
        successor = successor_def()

        traits = traits_def(IsTerminator(), BranchOpHasCanonicalizationPatterns())

        def __init__(self, dest: Block, *ops: Operation | SSAValue):
           super().__init__(operands=[[op for op in ops]], successors=[dest])
    ```

It can also have a region that contains other statements, for example, a function statement (simplified).

=== "Kirin"

    ```python
    @statement
    class Function(ir.Statement):
       name = "func"
       traits = frozenset({SSACFGRegion()}) # (1)!
       sym_name: str = info.attribute() # (2)!
       body: Region = info.region(multi=True) # (3)!
    ```

    1. The `traits` field contains the `SSACFGRegion` trait, which indicates that the region in the statement is a standard control-flow graph.
    2. The `sym_name` field is the name of the function. In the [`@statement`][kirin.decl.statement] decorator, if a field annotated with normal Python types (not an IR node, e.g [`ir.SSAValue`][kirin.ir.SSAValue], [`ir.Block`][kirin.ir.Block], [`ir.Region`][kirin.ir.Region]), it will be treated as a [`PyAttr`][kirin.ir.PyAttr] attribute.
    3. The `body` field is the region that contains the statements of the function. The field descriptor [`info.region`][kirin.decl.info.region] is used to specify this region can contain multiple blocks.


=== "MLIR"

    ```mlir
    def FuncOp : Func_Op<"func", [
        AffineScope, AutomaticAllocationScope,
        FunctionOpInterface, IsolatedFromAbove, OpAsmOpInterface
    ]> {
        let arguments = (ins SymbolNameAttr:$sym_name);
        let regions = (region AnyRegion:$body);

        let builders = [OpBuilder<(ins
           "StringRef":$name, "FunctionType":$type,
           CArg<"ArrayRef<NamedAttribute>", "{}">:$attrs,
           CArg<"ArrayRef<DictionaryAttr>", "{}">:$argAttrs)
        >];
        let hasCustomAssemblyFormat = 1;
    }
    ```

=== "xDSL"

    ```python
    @irdl_op_definition
    class FuncOp(IRDLOperation):
        name = "func.func"
        traits = traits_def(
            IsolatedFromAbove(), SymbolOpInterface(), FuncOpCallableInterface()
        )
        body = region_def()
        sym_name = prop_def(StringAttr)

        def __init__(
            self,
            name: str,
            region: Region | type[Region.DEFAULT] = Region.DEFAULT,
        ):
            properties: dict[str, Attribute | None] = {"sym_name": StringAttr(name)}
            super().__init__(properties=properties, regions=[region])
    ```

### Constructing a Statement

Statements can be constructed in similar ways to constructing a normal Python `dataclass`. Taking the previous
definitions as an example:

```python
from kirin.dialects.py.constant import Constant

lhs, rhs = Constant(1), Constant(2) # (1)!
add = Add(lhs.result, rhs=rhs.result) # (2)!
```

1. Two [`Constant`][kirin.dialect.py.constant.Constant] statements are created with the value `1` and `2`.
2. An [`Add`][kirin.decl.statement] statement is created with the `lhs` and `rhs` fields set to the results of the `lhs` and `rhs` statements. Like `@dataclass` unless specified by `kw_only=True`, the fields are positional.

## Block

A block is a sequence of statements that are executed in order. Optionally, a block can have arguments that are passed from the predecessor block and terminates with a terminator statement. Unlike [`ir.Statement`][kirin.ir.Statement], the [`ir.Block`][kirin.ir.Block] class is final and cannot be extended.

### Constructing a Block

`Block` takes a `Sequence` of statements as an argument, e.g a list of statements.

```python
from kirin import ir
ir.Block() # Block(_args=())
ir.Block([stmt_a, stmt_b])
```

continue the example from [Constructing a Statement](#constructing-a-statement), we can construct
a block like following:

```python
block = ir.Block()
arg_x = block.args.append_from(ir.types.Any)
arg_y = block.args.append_from(ir.types.Any)
block.stmts.append(Add(arg_x, arg_y))
```

!!! note
    Every IR node in Kirin has a pretty printer that can be used to print the node in a human-readable format. Just call [`.print`][kirin.print.Printable.print] method. In the above example, we have

    ```mlir
    ^0(%0, %1):
        %2 = add(lhs=%0, rhs=%1) : !py.int
    ```
    which is the pretty-printed version of the block. You may notice this is similar to MLIR text format, which is intentional.

## Region

A region is a sequence of blocks that are connected by control flow. A region can contain multiple blocks and can be nested within another region via statements that contain a region field. Unlike [`ir.Statement`][kirin.ir.Statement], the [`ir.Region`][kirin.ir.Region] class is final and cannot be extended.

### Constructing a Region

Continuing the example from [Constructing a Block](#constructing-a-block), we can construct a region like following:

```python
region = ir.Region([block])
```

pretty printing the region will give you

```mlir
{
  ^0(%1, %2):
  │ %0 = add(lhs=%1, rhs=%2) : !py.int
}
```

## SSA Value

An SSA value is a value that is assigned only once in the program. In Kirin IR, an SSA value is represented by the [`ir.SSAValue`][kirin.ir.SSAValue] class. Most of the time, one does not need to construct the SSA value directly, as it is automatically created when constructing a statement.

There are 3 types of SSA values:

- [`ir.SSAValue`][kirin.ir.SSAValue]: the base class of SSA values.
- [`ir.ResultValue`][kirin.ir.ResultValue]: SSA values that are the result of a statement, this object allows you to access the parent statement via [`result.owner`][kirin.ir.SSAValue.owner] property.
- [`ir.BlockArgument`][kirin.ir.BlockArgument]: SSA values that are the arguments of a block.
