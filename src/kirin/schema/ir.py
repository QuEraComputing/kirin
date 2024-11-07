from pydantic import BaseModel


class SSAValue(BaseModel):
    id: int
    name: str | None = None


class Statement(BaseModel):
    dialect: str
    name: str
    args: list[SSAValue]
    results: list[SSAValue]
    successors: list[int] = []
    regions: list["Region"] = []  # NOTE: stmt owns regions


class Block(BaseModel):
    id: int
    args: list[SSAValue] = []
    stmts: list[Statement] = []


class Region(BaseModel):
    blocks: list[Block] = []
