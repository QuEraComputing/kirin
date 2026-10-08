from kirin import ir, types
from kirin.dialects import py, scf, func, ilist
from kirin.rewrite.abc import RewriteRule, RewriteResult
from kirin.ir.nodes.stmt import Statement


class MapToForLoop(RewriteRule):
    """Rewrite `ilist.map` into an indexed loop that accumulates callback results.

    For example, rewrites:
    ```python
    ys = ilist.map(fn, xs)
    ```

    to:
    ```python
    ys = []
    for i in range(len(xs)):
        ys = ys + [fn(xs[i])]
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, ilist.Map):
            return RewriteResult()

        collection_type = node.collection.type
        result_type = node.result.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType)
            and isinstance(result_type, types.Generic)
            and result_type.is_subseteq(ilist.IListType)
        ):
            return RewriteResult()

        input_element_type = collection_type.vars[0]
        output_element_type = result_type.vars[0]
        accumulator_type = ilist.IListType[output_element_type, types.Any]

        length = py.Len(node.collection)
        zero = py.Constant(0)
        one = py.Constant(1)
        indices = ilist.stmts.Range(zero.result, length.result, one.result)
        empty = ilist.New((), elem_type=output_element_type)

        body = ir.Block(argtypes=(types.Int, accumulator_type))
        index, accumulated = body.args

        element = py.GetItem(node.collection, index)
        element.result.type = input_element_type

        call = func.Call(
            callee=node.fn,
            inputs=(element.result,),
            kwargs=(),
        )
        call.result.type = output_element_type

        singleton = ilist.New((call.result,), elem_type=output_element_type)
        appended = py.Add(accumulated, singleton.result)
        appended.result.type = accumulator_type

        for stmt in (element, call, singleton, appended, scf.Yield(appended.result)):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body), empty.result)
        loop.results[0].type = result_type

        for stmt in (length, zero, one, indices, empty, loop):
            stmt.insert_before(node)

        node.result.replace_by(loop.results[0])
        node.delete()
        return RewriteResult(has_done_something=True)


class FoldToForLoop(RewriteRule):
    """Rewrite folds into indexed loops.

    Foldl traverses forward with accumulator-first callbacks. Foldr traverses
    backward with element-first callbacks.

    For example, rewrites:
    ```python
    result = ilist.foldl(fn, xs, initial)
    ```

    to:
    ```python
    result = initial
    for i in range(len(xs)):
        result = fn(result, xs[i])
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, (ilist.Foldl, ilist.Foldr)):
            return RewriteResult()

        collection_type = node.collection.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType)
        ):
            return RewriteResult()

        element_type = collection_type.vars[0]
        accumulator_type = node.result.type

        length = py.Len(node.collection)
        setup: list[Statement] = [length]
        if isinstance(node, ilist.Foldr):
            one = py.Constant(1)
            last = py.Sub(length.result, one.result)
            last.result.type = types.Int
            negative_one = py.Constant(-1)
            setup.extend((one, last, negative_one))
            indices = ilist.stmts.Range(
                last.result, negative_one.result, negative_one.result
            )
        else:
            zero = py.Constant(0)
            one = py.Constant(1)
            setup.extend((zero, one))
            indices = ilist.stmts.Range(zero.result, length.result, one.result)

        body = ir.Block(argtypes=(types.Int, accumulator_type))
        index, accumulator = body.args

        element = py.GetItem(node.collection, index)
        element.result.type = element_type

        inputs = (
            (element.result, accumulator)
            if isinstance(node, ilist.Foldr)
            else (accumulator, element.result)
        )
        call = func.Call(callee=node.fn, inputs=inputs, kwargs=())
        call.result.type = accumulator_type

        for stmt in (element, call, scf.Yield(call.result)):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body), node.init)

        for stmt in (*setup, indices, loop):
            stmt.insert_before(node)

        node.result.replace_by(loop.results[0])
        node.delete()
        return RewriteResult(has_done_something=True)


class ScanToForLoop(RewriteRule):
    """Rewrite Scan into an indexed loop carrying state and an output list.

    Requires inferred input and result types.

    For example, rewrites:
    ```python
    result = ilist.scan(fn, xs, initial)
    ```

    to:
    ```python
    state = initial
    outputs = []
    for i in range(len(xs)):
        pair = fn(state, xs[i])
        state = pair[0]
        outputs = outputs + [pair[1]]
    result = (state, outputs)
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, ilist.Scan):
            return RewriteResult()

        collection_type = node.collection.type
        result_type = node.result.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType)
            and isinstance(result_type, types.Generic)
            and result_type.is_subseteq(types.Tuple)
            and len(result_type.vars) == 2
        ):
            return RewriteResult()

        state_type, output_type = result_type.vars
        if not (
            isinstance(output_type, types.Generic)
            and output_type.is_subseteq(ilist.IListType)
        ):
            return RewriteResult()

        output_element_type = output_type.vars[0]
        accumulated_type = ilist.IListType[output_element_type, types.Any]
        length = py.Len(node.collection)
        zero = py.Constant(0)
        one = py.Constant(1)
        indices = ilist.stmts.Range(zero.result, length.result, one.result)
        empty = ilist.New((), elem_type=output_element_type)

        body = ir.Block(argtypes=(types.Int, state_type, accumulated_type))
        index, state, accumulated = body.args
        element = py.GetItem(node.collection, index)
        element.result.type = collection_type.vars[0]
        call = func.Call(callee=node.fn, inputs=(state, element.result), kwargs=())
        call.result.type = types.Tuple[state_type, output_element_type]
        next_state = py.GetItem(call.result, zero.result)
        next_state.result.type = state_type
        value = py.GetItem(call.result, one.result)
        value.result.type = output_element_type
        singleton = ilist.New((value.result,), elem_type=output_element_type)
        appended = py.Add(accumulated, singleton.result)
        appended.result.type = accumulated_type

        for stmt in (
            element,
            call,
            next_state,
            value,
            singleton,
            appended,
            scf.Yield(next_state.result, appended.result),
        ):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body), node.init, empty.result)
        loop.results[1].type = output_type
        result = py.tuple.New((loop.results[0], loop.results[1]))
        result.result.type = result_type

        for stmt in (length, zero, one, indices, empty, loop, result):
            stmt.insert_before(node)

        node.result.replace_by(result.result)
        node.delete()
        return RewriteResult(has_done_something=True)


class ForEachToForLoop(RewriteRule):
    """Rewrite ForEach into an indexed loop preserving callback order and effects.

    For example, rewrites:
    ```python
    ilist.for_each(fn, xs)
    ```

    to:
    ```python
    for i in range(len(xs)):
        fn(xs[i])
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, ilist.ForEach):
            return RewriteResult()

        collection_type = node.collection.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType)
        ):
            return RewriteResult()

        length = py.Len(node.collection)
        zero = py.Constant(0)
        one = py.Constant(1)
        indices = ilist.stmts.Range(zero.result, length.result, one.result)
        body = ir.Block(argtypes=(types.Int,))
        element = py.GetItem(node.collection, body.args[0])
        element.result.type = collection_type.vars[0]
        call = func.Call(callee=node.fn, inputs=(element.result,), kwargs=())
        callback_type = node.fn.type
        if isinstance(callback_type, types.FunctionType):
            if callback_type.return_type is not None:
                call.result.type = callback_type.return_type
        for stmt in (element, call, scf.Yield()):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body))
        for stmt in (length, zero, one, indices, loop):
            stmt.insert_before(node)
        node.delete()
        return RewriteResult(has_done_something=True)


class AnyToForLoop(RewriteRule):
    """Rewrite Any into an OR reduction over an immutable boolean list.

    For example, rewrites:
    ```python
    result = ilist.any(xs)
    ```

    to:
    ```python
    result = False
    for i in range(len(xs)):
        value = xs[i]
        result = result or value
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, ilist.stmts.Any):
            return RewriteResult()

        collection_type = node.collection.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType[types.Bool])
        ):
            return RewriteResult()

        length = py.Len(node.collection)
        zero = py.Constant(0)
        one = py.Constant(1)
        indices = ilist.stmts.Range(zero.result, length.result, one.result)
        initial = py.Constant(False)
        body = ir.Block(argtypes=(types.Int, types.Bool))
        index, accumulated = body.args
        element = py.GetItem(node.collection, index)
        element.result.type = types.Bool
        combined = py.Or(accumulated, element.result)
        for stmt in (element, combined, scf.Yield(combined.result)):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body), initial.result)
        for stmt in (length, zero, one, indices, initial, loop):
            stmt.insert_before(node)
        node.result.replace_by(loop.results[0])
        node.delete()
        return RewriteResult(has_done_something=True)


class AllToForLoop(RewriteRule):
    """Rewrite All into an AND reduction over an immutable boolean list.

    For example, rewrites:
    ```python
    result = ilist.all(xs)
    ```

    to:
    ```python
    result = True
    for i in range(len(xs)):
        value = xs[i]
        result = result and value
    ```
    """

    def rewrite_Statement(self, node: Statement) -> RewriteResult:
        if not isinstance(node, ilist.stmts.All):
            return RewriteResult()

        collection_type = node.collection.type
        if not (
            isinstance(collection_type, types.Generic)
            and collection_type.is_subseteq(ilist.IListType[types.Bool])
        ):
            return RewriteResult()

        length = py.Len(node.collection)
        zero = py.Constant(0)
        one = py.Constant(1)
        indices = ilist.stmts.Range(zero.result, length.result, one.result)
        initial = py.Constant(True)
        body = ir.Block(argtypes=(types.Int, types.Bool))
        index, accumulated = body.args
        element = py.GetItem(node.collection, index)
        element.result.type = types.Bool
        combined = py.And(accumulated, element.result)
        for stmt in (element, combined, scf.Yield(combined.result)):
            body.stmts.append(stmt)

        loop = scf.For(indices.result, ir.Region(body), initial.result)
        for stmt in (length, zero, one, indices, initial, loop):
            stmt.insert_before(node)
        node.result.replace_by(loop.results[0])
        node.delete()
        return RewriteResult(has_done_something=True)
