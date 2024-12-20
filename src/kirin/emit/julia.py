from typing import IO, TypeVar

from kirin.ir.nodes.block import Block

from .str import EmitStr, EmitStrFrame

IO_t = TypeVar("IO_t", bound=IO)


class EmitJulia(EmitStr[IO_t]):
    keys = ["emit.julia"]

    def emit_block_begin(self, frame: EmitStrFrame, block: Block) -> None:
        block_id = self.block_id[block]
        frame.block_ref[block] = block_id
        self.newline(frame)
        self.write(f"@label {block_id};")
