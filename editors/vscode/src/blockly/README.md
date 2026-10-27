# Blockly PLC Editor

Visual programming editor for IEC 61131-3 PLC programs using Google Blockly.

## Overview

A Blockly program is a `.blockly.json` file. The editor draws and edits it with the Blockly
library; what the blocks mean as Structured Text is decided by the truST language server
(`crates/trust-visual/src/blockly`, reached through the `trust-lsp.visual*` commands).

Every Blockly program executes as Structured Text (specification 17 §3): saving the file writes
a sibling companion `<name>.st` holding the program as `FUNCTION_BLOCK FB_<name>_BLOCKLY`, and a
runtime entry `<name>.visual.runtime.st` holding a `PROGRAM` and `CONFIGURATION` that run it.
The editor has no runtime controls of its own: run the project from the truST sidebar and watch
values in Live Values.

## Getting Started

### Creating a New Blockly Program

1. Open Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`)
2. Run: `truST: New Blockly Program`
3. Enter a program name

A new `.blockly.json` file opens in the Blockly editor and its companion is written beside it.

### Importing an Existing Program

1. Open Command Palette
2. Run: `truST: Import Blockly Program`
3. Select a `.blockly.json` file to import

## Using the Editor

- **Validate** shows the function block the language server generates and its warnings.
- **Generate ST** shows the companion and asks whether to write it (with the runtime entry).
- **Preview ST** shows the last generated code without writing anything.
- **Save** writes the workspace; the companion follows on save while
  `trust.visual.generateStructuredTextCompanions` is on.

## Blocks the companion expresses

| Block | Structured Text |
|-------|-----------------|
| `controls_if` (`IF0`/`DO0`, or `IF`/`DO`) | `IF … THEN … END_IF;` |
| `controls_whileUntil` (`WHILE` or `UNTIL`) | `WHILE … DO … END_WHILE;` (`UNTIL` negates the condition) |
| `variables_set` / `variables_get` | assignment / variable |
| `io_digital_write` / `io_digital_read` | assignment to / read of a direct address |
| `logic_compare` | `(A = B)`, `<>`, `<`, `<=`, `>`, `>=` |
| `math_arithmetic` | `(A + B)`, `-`, `*`, `/`, `**` |
| `logic_boolean`, `math_number`, `text` | literals (text as an IEC string) |

Any other block becomes a comment and a warning. A declared variable without a type takes the
type of the first value assigned to it (`BOOL`, `INT` or `REAL`), and `BOOL` otherwise. Direct
addresses become globals of the runtime entry, typed by their size (specification 03 §5).

## File Format

```json
{
  "blocks": {
    "languageVersion": 0,
    "blocks": [
      {
        "type": "controls_if",
        "id": "block_id_1",
        "x": 50,
        "y": 50,
        "inputs": {
          "IF0": {
            "block": {
              "type": "logic_compare",
              "id": "block_id_2",
              "fields": { "OP": "EQ" }
            }
          }
        }
      }
    ]
  },
  "variables": [
    {
      "id": "var_1",
      "name": "counter",
      "type": "INT"
    }
  ],
  "metadata": {
    "name": "MyProgram",
    "description": "Example PLC program",
    "version": "1.0.0"
  }
}
```

## Files

```
editors/vscode/src/blockly/
├── blocklyEditor.ts          # Custom editor provider: forwards Validate/Generate to the language server
├── newBlockly.ts             # New program command
├── importBlockly.ts          # Import program command
├── uriUtils.ts               # URI helpers
└── webview/                  # React + Blockly webview (drawing and editing only)
crates/trust-visual/src/blockly/  # The Structured Text lowering and its tests
```

Build the webview with `npm run build:blockly`.
