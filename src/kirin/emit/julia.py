from typing import IO, TypeVar

from kirin.emit.abc import EmitFrame
from kirin.ir.nodes.block import Block

from .str import EmitStr

IO_t = TypeVar("IO_t", bound=IO)


class EmitJulia(EmitStr[IO_t]):
    keys = ["emit.julia"]

    def emit_block_header(self, frame: EmitFrame[str], block: Block) -> str:
        block_id = self.block_id[block]
        self.newline(frame)
        self.write(f"@label {block_id};")
        return block_id
