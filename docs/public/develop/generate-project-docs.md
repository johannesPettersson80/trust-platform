# Generate Project Docs

truST can generate project-facing documentation from tagged ST comments.

## Core command

```bash
trust-dev docs --project ./my-plc --format both --out-dir ./docs/api
```

## Supported doc tags

| Tag | Meaning |
| --- | --- |
| `@brief` | short item summary |
| `@param <name> ...` | parameter documentation |
| `@return ...` | return-value documentation for returning items |

## Example source comment block

```st
// @brief Adds two numbers.
// @param A Left-hand value.
// @param B Right-hand value.
// @return Sum value.
FUNCTION Add : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
```

## Example output shape

Generated docs surface:

- qualified item name
- source file and line
- brief description
- parameter table
- return description when present

## CI example

```bash
trust-dev docs --project ./my-plc --format markdown --out-dir ./docs/api
```

Use this together with:

- [CI/CD](../operate/ci-cd.md)
- [trust-dev CLI](../reference/cli/trust-dev.md)
