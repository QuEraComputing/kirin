from __future__ import annotations

from typing import TypeVar

from kirin import ir, interp, lattice

dialect = ir.Dialect("ssacfg")


@dialect.register(key="main")
class Concrete(interp.MethodTable):

    @interp.impl(ir.SSACFG())
    def ssacfg(self, interp_: interp.Interpreter, frame: interp.Frame, node: ir.Region):
        block = node.blocks[0]
        block_inputs = frame.get_values(block.args)
        while block is not None:
            frame.current_block = block
            frame.set_values(block.args, block_inputs)
            for stmt in block.stmts:
                frame.current_stmt = stmt
                stmt_results = interp_.frame_eval(frame, stmt)
                match stmt_results:
                    case tuple():
                        frame.set_values(stmt._results, stmt_results)
                    case None:
                        continue
                    case interp.Successor(block, block_inputs):
                        pass
                    case interp.ReturnValue():
                        return stmt_results  # terminate the call frame
                    case interp.YieldValue(values):
                        return values  # terminate the region
        return


@dialect.register(key="abstract")
class Abstract(interp.MethodTable):

    FrameType = TypeVar("FrameType", bound=interp.AbstractFrame)
    LatticeType = TypeVar("LatticeType", bound=lattice.BoundedLattice)

    @interp.impl(ir.SSACFG())
    def ssacfg(
        self,
        interp_: interp.AbstractInterpreter[FrameType, LatticeType],
        frame: FrameType,
        node: ir.Region,
    ):
        result = None
        frame.worklist.append(
            interp.Successor(node.blocks[0], *frame.get_values(node.blocks[0].args))
        )
        while (succ := frame.worklist.pop()) is not None:
            visited = frame.visited.setdefault(succ.block, set())
            if succ in visited:
                continue

            block_result = self.run_succ(interp_, frame, succ)
            if len(frame.visited[succ.block]) < 128:
                frame.visited[succ.block].add(succ)
            else:
                continue

            if isinstance(block_result, interp.Successor):
                raise interp.InterpreterError(
                    "unexpected successor, successors should be in worklist"
                )

            result = interp_.join_results(result, block_result)

        if isinstance(result, interp.YieldValue):
            return result.values
        return result

    def run_succ(
        self,
        interp_: interp.AbstractInterpreter[FrameType, LatticeType],
        frame: FrameType,
        succ: interp.Successor,
    ) -> interp.SpecialValue[LatticeType]:
        frame.current_block = succ.block
        frame.set_values(succ.block.args, succ.block_args)
        for stmt in succ.block.stmts:
            frame.current_stmt = stmt
            stmt_results = interp_.frame_eval(frame, stmt)
            if isinstance(stmt_results, tuple):
                frame.set_values(stmt._results, stmt_results)
            elif stmt_results is None:
                continue  # empty result
            else:  # terminate
                return stmt_results
        return None
