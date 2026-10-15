"""Interactive HTML inspection for Kirin IR.

The renderer is intentionally dependency-free so an exported file can be
opened locally without a running server.  It is a debugging view: it does not
attempt to edit or execute the IR.
"""

from __future__ import annotations

import html
import json
from typing import TYPE_CHECKING, Any
from pathlib import Path
from collections.abc import Mapping

from kirin.ir.ssa import SSAValue, ResultValue, BlockArgument
from kirin.idtable import IdTable
from kirin.ir.nodes.stmt import Statement
from kirin.ir.nodes.block import Block
from kirin.ir.nodes.region import Region

if TYPE_CHECKING:
    from kirin.source import SourceInfo
    from kirin.ir.method import Method


def to_html(
    node: Method | Statement | Region | Block,
    *,
    analysis: Mapping[SSAValue, Any] | None = None,
    title: str | None = None,
) -> str:
    """Render Kirin IR as a self-contained interactive HTML document.

    Hovering an SSA value shows its producer, type, uses, hints, and an
    optional analysis fact. Hovering a statement shows its IR class, dialect,
    operands, results, attributes, traits, and source span.
    """

    return _Inspector(node, analysis=analysis, title=title).render()


def write_html(
    node: Method | Statement | Region | Block,
    file: str | Path,
    *,
    analysis: Mapping[SSAValue, Any] | None = None,
    title: str | None = None,
) -> Path:
    """Write an interactive HTML inspection page and return its path."""

    path = Path(file)
    path.write_text(to_html(node, analysis=analysis, title=title), encoding="utf-8")
    return path


class _Inspector:
    def __init__(
        self,
        node: Method | Statement | Region | Block,
        *,
        analysis: Mapping[SSAValue, Any] | None,
        title: str | None,
    ) -> None:
        self.node = node
        self.analysis = analysis
        self.title = title or self._default_title(node)
        self.ssa_labels: IdTable[SSAValue] = IdTable()
        self.ssa_ids: dict[SSAValue, str] = {}
        self.statement_ids: dict[Statement, str] = {}
        self.block_ids: dict[Block, str] = {}
        self.ssa_details: dict[str, dict[str, Any]] = {}
        self.statement_details: dict[str, dict[str, Any]] = {}

    @staticmethod
    def _default_title(node: Method | Statement | Region | Block) -> str:
        sym_name = getattr(node, "sym_name", None)
        if sym_name is not None:
            return f"Kirin IR: {sym_name or '<lambda>'}"
        return "Kirin IR Inspector"

    def render(self) -> str:
        body = self._render_node(self.node)
        data = json.dumps(
            {"ssa": self.ssa_details, "statements": self.statement_details},
            ensure_ascii=True,
        )
        return _PAGE.format(
            title=html.escape(self.title),
            body=body,
            data=data.replace("</", "<\\/"),
        )

    def _render_node(self, node: Method | Statement | Region | Block) -> str:
        if isinstance(node, Statement):
            return self._render_statement(node, 0)
        if isinstance(node, Region):
            return self._render_region(node, 0)
        if isinstance(node, Block):
            return self._render_block(node, 0)
        return self._render_statement(node.code, 0)

    def _ssa_id(self, value: SSAValue) -> str:
        if value not in self.ssa_ids:
            self.ssa_ids[value] = f"ssa-{len(self.ssa_ids)}"
        return self.ssa_ids[value]

    def _ssa_label(self, value: SSAValue) -> str:
        return self.ssa_labels[value]

    def _statement_id(self, statement: Statement) -> str:
        if statement not in self.statement_ids:
            self.statement_ids[statement] = f"stmt-{len(self.statement_ids)}"
        return self.statement_ids[statement]

    def _block_id(self, block: Block) -> str:
        if block not in self.block_ids:
            self.block_ids[block] = f"block-{len(self.block_ids)}"
        return self.block_ids[block]

    def _render_ssa(self, value: SSAValue) -> str:
        key = self._ssa_id(value)
        label = self._ssa_label(value)
        self._add_ssa_details(value)
        return (
            f'<span class="ssa" data-ssa="{key}" tabindex="0">'
            f"{html.escape(label)}</span>"
        )

    def _render_statement(self, statement: Statement, depth: int) -> str:
        statement_id = self._statement_id(statement)
        self._add_statement_details(statement)
        results = ", ".join(self._render_ssa(value) for value in statement.results)
        prefix = f'<span class="results">{results} = </span>' if results else ""
        operation = html.escape(self._operation_name(statement))
        args = self._render_arguments(statement)
        attributes = self._render_attributes(statement)
        result_types = ", ".join(
            html.escape(_safe_repr(value.type)) for value in statement.results
        )
        type_suffix = (
            f' <span class="types">: {result_types}</span>' if result_types else ""
        )
        source = self._source_label(statement.source)
        source_suffix = (
            f' <span class="source">{html.escape(source)}</span>' if source else ""
        )
        indent = depth * 20
        line = (
            f'<div class="stmt" data-stmt="{statement_id}" tabindex="0" '
            f'style="--indent: {indent}px">{prefix}'
            f'<span class="operation">{operation}</span>({args}){attributes}'
            f"{type_suffix}{source_suffix}</div>"
        )
        if not statement.regions:
            return line

        regions = "".join(
            self._render_region(region, depth + 1) for region in statement.regions
        )
        return f'{line}<div class="regions">{regions}</div>'

    def _render_region(self, region: Region, depth: int) -> str:
        if not region.blocks:
            return '<div class="empty-region">{{}}</div>'
        return "".join(self._render_block(block, depth) for block in region.blocks)

    def _render_block(self, block: Block, depth: int) -> str:
        block_id = self._block_id(block)
        args = ", ".join(
            f'{self._render_ssa(arg)} <span class="types">: '
            f"{html.escape(_safe_repr(arg.type))}</span>"
            for arg in block.args
        )
        header = ""
        has_multiple_blocks = block.parent is not None and len(block.parent.blocks) > 1
        if args or has_multiple_blocks:
            header = (
                f'<div class="block" id="{block_id}" style="--indent: {depth * 20}px">'
                f'<span class="block-label">^{block_id}</span>({args}):</div>'
            )
        statements = "".join(
            self._render_statement(statement, depth + 1) for statement in block.stmts
        )
        return f"{header}{statements}"

    def _render_arguments(self, statement: Statement) -> str:
        names = self._argument_names(statement)
        values: list[str] = []
        for index, value in enumerate(statement.args):
            values.append(
                f'<span class="arg-name">{html.escape(names.get(index, f"arg{index}"))}</span>='
                f"{self._render_ssa(value)}"
            )
        return ", ".join(values)

    def _render_attributes(self, statement: Statement) -> str:
        if not statement.attributes:
            return ""
        rendered = ", ".join(
            f"{html.escape(name)}={html.escape(_safe_repr(value))}"
            for name, value in statement.attributes.items()
        )
        return f' <span class="attributes">{{{rendered}}}</span>'

    @staticmethod
    def _argument_names(statement: Statement) -> dict[int, str]:
        names: dict[int, str] = {}
        for name, slice_ in statement._name_args_slice.items():
            if isinstance(slice_, int):
                names[slice_] = name
            else:
                start, stop, step = slice_.indices(len(statement.args))
                for index in range(start, stop, step):
                    names[index] = f"{name}[{index - start}]"
        return names

    @staticmethod
    def _operation_name(statement: Statement) -> str:
        dialect = statement.dialect.name if statement.dialect else ""
        return f"{dialect + '.' if dialect else ''}{statement.name}"

    def _add_ssa_details(self, value: SSAValue) -> None:
        key = self._ssa_id(value)
        if key in self.ssa_details:
            return

        details: dict[str, Any] = {
            "Kind": type(value).__name__,
            "Type": _safe_repr(value.type),
            "Hints": {name: _safe_repr(hint) for name, hint in value.hints.items()},
        }
        if isinstance(value, ResultValue):
            details["Producer"] = {
                "statement": self._statement_id(value.owner),
                "operation": self._operation_name(value.owner),
                "result index": value.index,
            }
        elif isinstance(value, BlockArgument):
            details["Producer"] = {
                "block": self._block_id(value.owner),
                "argument index": value.index,
            }
        else:
            details["Producer"] = _safe_repr(value.owner)

        uses = sorted(
            value.uses, key=lambda use: (self._statement_id(use.stmt), use.index)
        )
        details["Uses"] = [
            {
                "statement": self._statement_id(use.stmt),
                "operation": self._operation_name(use.stmt),
                "operand index": use.index,
            }
            for use in uses
        ]
        if self.analysis is not None and value in self.analysis:
            details["Analysis"] = _safe_repr(self.analysis[value])
        self.ssa_details[key] = details

    def _add_statement_details(self, statement: Statement) -> None:
        key = self._statement_id(statement)
        if key in self.statement_details:
            return

        argument_names = self._argument_names(statement)
        details: dict[str, Any] = {
            "Class": f"{type(statement).__module__}.{type(statement).__qualname__}",
            "Dialect": statement.dialect.name if statement.dialect else None,
            "Operation": self._operation_name(statement),
            "Operands": [
                {
                    "name": argument_names.get(index, f"arg{index}"),
                    "ssa": self._ssa_label(value),
                    "type": _safe_repr(value.type),
                }
                for index, value in enumerate(statement.args)
            ],
            "Results": [
                {
                    "ssa": self._ssa_label(value),
                    "type": _safe_repr(value.type),
                    "uses": len(value.uses),
                }
                for value in statement.results
            ],
            "Attributes": {
                name: _safe_repr(value) for name, value in statement.attributes.items()
            },
            "Traits": [type(trait).__name__ for trait in statement.traits],
            "Regions": len(statement.regions),
            "Successors": [self._block_id(block) for block in statement.successors],
        }
        if source := self._source_details(statement.source):
            details["Source"] = source
        self.statement_details[key] = details

    @staticmethod
    def _source_label(source: SourceInfo | None) -> str | None:
        if source is None:
            return None
        line = source.lineno + source.lineno_begin
        location = f"{source.file or '<unknown>'}:{line}:{source.col_offset + source.col_indent}"
        return location

    def _source_details(self, source: SourceInfo | None) -> dict[str, Any] | None:
        label = self._source_label(source)
        if source is None or label is None:
            return None
        details: dict[str, Any] = {"location": label}
        if snippet := _read_source_snippet(source):
            details["snippet"] = snippet
        return details


def _safe_repr(value: object) -> str:
    try:
        return repr(value)
    except Exception:
        return f"<{type(value).__name__}>"


def _read_source_snippet(source: SourceInfo) -> str | None:
    if not source.file:
        return None
    try:
        lines = Path(source.file).read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError):
        return None

    first = source.lineno + source.lineno_begin
    last = (source.end_lineno or source.lineno) + source.lineno_begin
    if first < 1 or last < first or first > len(lines):
        return None
    return "\n".join(lines[first - 1 : min(last, len(lines))])


_PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{
  color-scheme: dark;
  --page: #151817;
  --panel: #1d2220;
  --panel-strong: #242b28;
  --line: #38403b;
  --text: #edf1ed;
  --muted: #aab4ae;
  --accent: #e7b85a;
  --ssa: #8ed6c4;
  --operation: #8fb8e8;
  --type: #d8a4d5;
  --source: #8fa697;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  background: var(--page);
  color: var(--text);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 13px;
  line-height: 1.55;
}}
header {{
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  min-height: 42px;
  padding: 0 18px;
  border-bottom: 1px solid var(--line);
  background: #181c1a;
  color: var(--text);
  font-family: ui-sans-serif, system-ui, sans-serif;
  font-size: 14px;
  font-weight: 600;
}}
main {{
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(270px, 34vw);
  min-height: calc(100vh - 42px);
}}
#ir {{ padding: 18px 24px 48px; overflow: auto; }}
#inspector {{
  position: sticky;
  top: 42px;
  height: calc(100vh - 42px);
  overflow: auto;
  border-left: 1px solid var(--line);
  background: var(--panel);
  padding: 16px;
}}
.inspector-title {{
  margin: 0 0 12px;
  color: var(--muted);
  font-family: ui-sans-serif, system-ui, sans-serif;
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
}}
.empty {{ color: var(--muted); font-family: ui-sans-serif, system-ui, sans-serif; }}
.stmt, .block {{
  padding: 2px 8px 2px calc(var(--indent) + 8px);
  border-left: 2px solid transparent;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}}
.stmt:hover, .stmt:focus, .stmt.active {{
  outline: none;
  border-left-color: var(--accent);
  background: #2a302c;
}}
.regions {{ margin: 2px 0; border-left: 1px solid #303732; }}
.block {{ color: var(--muted); }}
.block-label {{ color: #a8c8ef; }}
.ssa {{ color: var(--ssa); cursor: pointer; border-radius: 2px; padding: 0 1px; }}
.ssa:hover, .ssa:focus, .ssa.active {{
  outline: none;
  color: #101413;
  background: var(--ssa);
}}
.operation {{ color: var(--operation); font-weight: 600; }}
.arg-name {{ color: #d5dbd7; }}
.results {{ color: var(--text); }}
.types {{ color: var(--type); }}
.attributes {{ color: #c8c4a1; }}
.source {{ color: var(--source); }}
.empty-region {{ color: var(--muted); padding-left: 20px; }}
.detail-group {{ margin: 0 0 16px; }}
.detail-key {{ color: var(--accent); margin: 0 0 4px; }}
.detail-value {{ color: var(--text); white-space: pre-wrap; overflow-wrap: anywhere; }}
.detail-list {{ margin: 0; padding-left: 18px; }}
pre {{
  margin: 6px 0 0;
  padding: 10px;
  border: 1px solid var(--line);
  background: #161a18;
  color: #dce5df;
  overflow: auto;
  white-space: pre-wrap;
}}
@media (max-width: 760px) {{
  main {{ grid-template-columns: 1fr; }}
  #inspector {{ position: static; height: auto; border-left: 0; border-top: 1px solid var(--line); }}
}}
</style>
</head>
<body>
<header>{title}</header>
<main>
  <section id="ir" aria-label="Kirin IR">{body}</section>
  <aside id="inspector" aria-live="polite"><div class="empty">Select an IR element</div></aside>
</main>
<script id="ir-data" type="application/json">{data}</script>
<script>
const data = JSON.parse(document.getElementById("ir-data").textContent);
const inspector = document.getElementById("inspector");
let active = [];

function clearActive() {{
  active.forEach((element) => element.classList.remove("active"));
  active = [];
}}

function escapeHtml(value) {{
  return String(value).replace(/[&<>"']/g, (char) => ({{
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"
  }}[char]));
}}

function render(value) {{
  if (value === null || value === undefined) return '<span class="detail-value">none</span>';
  if (Array.isArray(value)) {{
    if (!value.length) return '<span class="detail-value">none</span>';
    return '<ul class="detail-list">' + value.map((item) => '<li>' + render(item) + '</li>').join('') + '</ul>';
  }}
  if (typeof value === "object") {{
    const entries = Object.entries(value);
    if (!entries.length) return '<span class="detail-value">none</span>';
    return entries.map(([key, item]) =>
      '<div class="detail-group"><div class="detail-key">' + escapeHtml(key) +
      '</div><div class="detail-value">' + render(item) + '</div></div>'
    ).join('');
  }}
  return escapeHtml(value);
}}

function show(kind, id) {{
  clearActive();
  const selector = kind === "ssa" ? '[data-ssa="' + id + '"]' : '[data-stmt="' + id + '"]';
  active = Array.from(document.querySelectorAll(selector));
  active.forEach((element) => element.classList.add("active"));
  const details = kind === "ssa" ? data.ssa[id] : data.statements[id];
  inspector.innerHTML = '<div class="inspector-title">' + (kind === "ssa" ? "SSA value" : "Statement") + '</div>' + render(details);
}}

document.addEventListener("mouseover", (event) => {{
  const ssa = event.target.closest("[data-ssa]");
  if (ssa) return show("ssa", ssa.dataset.ssa);
  const statement = event.target.closest("[data-stmt]");
  if (statement) show("statement", statement.dataset.stmt);
}});
document.addEventListener("focusin", (event) => {{
  const ssa = event.target.closest("[data-ssa]");
  if (ssa) return show("ssa", ssa.dataset.ssa);
  const statement = event.target.closest("[data-stmt]");
  if (statement) show("statement", statement.dataset.stmt);
}});
</script>
</body>
</html>
"""
