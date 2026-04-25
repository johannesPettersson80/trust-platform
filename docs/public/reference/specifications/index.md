# Specifications

The public reference pages below render the maintained spec sources under
`docs/specs/`. That keeps the full spec text searchable in the docs site
instead of sending readers to GitHub.

## Which spec should you open?

| Question | Open |
| --- | --- |
| “What does IEC Structured Text itself allow?” | the language chapters `01` through `09` |
| “How does truST execute ST programs?” | [10 Runtime Semantics](10-runtime-semantics.md) |
| “How is the runtime platform structured?” | [11 Runtime Engine](11-runtime-engine.md) |
| “What does the bytecode container look like?” | [12 Bytecode](12-bytecode.md) |
| “How does debugging behave?” | [13 Debug Adapter](13-debug-adapter.md) |
| “How does the editor/LSP layer behave?” | [14 LSP](14-lsp.md) |
| “How does ladder fit into truST?” | [15 Ladder Diagram](15-ladder-diagram.md) and [16 Ladder Profile truST](16-ladder-profile-trust.md) |
| “How do the visual editors map onto one runtime model?” | [17 Visual Editors Runtime Unification](17-visual-editors-runtime-unification.md) |
| “What is the current SFC scope?” | [SFC Profile](sfc-profile.md) |
| “How are CONFIGURATION / RESOURCE / TASK declarations modeled?” | [18 Configurations, Resources, and Tasks](18-configurations-resources-tasks.md) |
| “What files make up a truST project?” | [19 Project Model](19-project-model.md) |
| “What is the machine-facing runtime contract?” | [20 Agent API v1](20-agent-api-v1.md) |
| “What is the deterministic harness wire protocol?” | [21 Harness Protocol](21-harness-protocol.md) |

Use specification pages for exact behavior and contracts. Use [Program](../../develop/index.md)
for task-oriented programming docs.

## Specification Set Overview

### Language

- [01 Lexical Elements](01-lexical-elements.md)
- [02 Data Types](02-data-types.md)
- [03 Variables](03-variables.md)
- [04 POU Declarations](04-pou-declarations.md)
- [05 Expressions](05-expressions.md)
- [06 Statements](06-statements.md)
- [07 Standard Functions](07-standard-functions.md)
- [08 Standard Function Blocks](08-standard-function-blocks.md)
- [09 Semantic Rules](09-semantic-rules.md)

### Runtime

- [10 Runtime Semantics](10-runtime-semantics.md)
- [11 Runtime Engine](11-runtime-engine.md)
- [12 Bytecode](12-bytecode.md)

### Tooling

- [13 Debug Adapter](13-debug-adapter.md)
- [14 LSP](14-lsp.md)

### Visual Editors

- [15 Ladder Diagram](15-ladder-diagram.md)
- [16 Ladder Profile truST](16-ladder-profile-trust.md)
- [17 Visual Editors Runtime Unification](17-visual-editors-runtime-unification.md)
- [SFC Profile](sfc-profile.md)

### Integration

- [18 Configurations, Resources, and Tasks](18-configurations-resources-tasks.md)
- [19 Project Model](19-project-model.md)
- [20 Agent API v1](20-agent-api-v1.md)
- [21 Harness Protocol](21-harness-protocol.md)

## Full Source Index

--8<-- "docs/specs/README.md:3"
