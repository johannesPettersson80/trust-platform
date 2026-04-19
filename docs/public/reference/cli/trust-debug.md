# `trust-debug`

`trust-debug` is the Debug Adapter Protocol bridge used by editor integrations.

## Transport

- stdio

## What it does

`trust-debug` starts a debug adapter session around the truST runtime/debug
contracts so editors can drive breakpoints, stepping, and variable inspection
through DAP.

## Typical users

- VS Code debug sessions
- other DAP-capable editors or tools

## Typical launch shape

This tool is usually started by an editor, not by typing directly into a shell.
The editor launches `trust-debug` over stdio and speaks DAP to it.

## When to read this page

- you are wiring a new editor integration
- you want to understand the adapter boundary
- you need to troubleshoot launch/attach behavior

## Notes

- it complements `trust-lsp`; it does not replace it
- for day-to-day debugging flow, see the runtime-panel guide first

## Related

- [Debugging And Runtime Panel](../../operate/debugging-and-runtime-panel.md)
- [trust-lsp](trust-lsp.md)
- [trust-runtime ctl](trust-runtime.md)
