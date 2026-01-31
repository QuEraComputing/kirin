## Polish Checklist

### High
- [x] Ensure `HasRecursiveParser` derive does not require `WithAbstractSyntaxTree` (or emit a clear diagnostic when only the parser is derived). Affects `crates/kirin-chumsky-format/src/generate/parser.rs`.

### Medium
- [x] Align region parsing with spec: require `{ ... }` for regions, or update the spec to match `region()` behavior in `crates/kirin-chumsky/src/parsers.rs`.
- [x] Define list/optional syntax in format strings or reject `Vec`/`Option` fields for format-derived parsers. Affects `crates/kirin-chumsky-format/src/generate/parser.rs`.
- [x] Disallow missing fields in format strings (avoid `Default::default()` fallback), or make defaults explicit/opt-in. Affects `crates/kirin-chumsky-format/src/generate/parser.rs`.

### Low
- [x] Reject duplicate default occurrences for a field (or enforce equality) instead of ignoring later occurrences. Affects `crates/kirin-chumsky-format/src/generate/parser.rs`.
- [x] Emit a diagnostic when `:name` or `:type` is used on non-SSA fields (blocks/regions/successors/value types). Affects `crates/kirin-chumsky-format/src/generate/parser.rs`.
- [x] Align block header syntax: require `()` even for zero args or update spec to allow bare `^bb0`. Affects `crates/kirin-chumsky/src/parsers.rs`.
- [x] Add escaping for literal `{` / `}` in format strings (e.g., `{{` / `}}`) or document the limitation. Affects `crates/kirin-chumsky-format/src/format.rs`.
- [x] Add runtime API unit tests (identifiers, SSA, blocks/regions, function types). Affects `crates/kirin-chumsky/src/tests.rs`.
