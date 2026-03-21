# <Dialect Name> Dialect Spec

## Domain

<What domain does this dialect target? 1-2 sentences.>

## Type System

<Types this dialect introduces or reuses. For each type:>

### <TypeName>
- Rust representation: <enum variant or struct>
- Text format: `!<namespace>.<type>`
- Lattice position: <bottom / top / intermediate>
- Placeholder: <what placeholder() returns>

## Operations

<For each operation, fill in all fields. Do not skip edge cases.>

### <operation name>

**Text format:**
```
%result = <namespace>.<op> %operand1, %operand2 -> <result-type>
```

**Operands:**
| Name | Type | Description |
|------|------|-------------|
| %operand1 | <type> | <what it represents> |

**Results:**
| Name | Type | Description |
|------|------|-------------|
| %result | <type> | <what it produces> |

**Attributes:**
- terminator: no
- pure: <yes/no>
- speculatable: <yes/no>
- constant: no

**Semantics:**
<Mathematical definition. What does this operation compute?>

**Type rule:**
```
<input types> -> <output types>
```

**Edge cases:**
- <boundary condition 1: what happens?>
- <boundary condition 2: what happens?>
- <precondition: what if violated?>
- <interaction with other ops: any surprising behavior?>

## Example Programs

<2-3 complete examples showing operations composed together. These become test cases.>

### Example 1: <name>

```kirin
<complete program text that should parse and roundtrip>
```

**Expected behavior:** <what the interpreter should produce>

### Example 2: <name>

```kirin
<complete program text>
```

**Expected behavior:** <what the interpreter should produce>

## Open Questions

<Anything unresolved that needs discussion during spec review.>
