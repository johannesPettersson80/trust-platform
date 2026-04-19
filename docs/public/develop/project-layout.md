# Project Layout

This page documents the recommended truST project shape, reusable-library
layout, and build/runtime flow from the maintained developer guide.

## Canonical project tree

```text
my-plc/
  runtime.toml
  io.toml
  trust-lsp.toml
  program.stbc
  src/
    Main.st
    Configuration.st
```

## How the files relate

| File or folder | Answers which question? |
| --- | --- |
| `src/` | what logic do I want to run? |
| `runtime.toml` | how should the runtime behave? |
| `io.toml` | what backend or device plane touches `%I/%Q`? |
| `trust-lsp.toml` | what editor/project/dependency rules apply? |
| `program.stbc` | what executable bytecode did the build produce? |

## Reusable library shape

Keep project-owned ST in the project `src/`. Put reusable packages in their own
package root and reference them through `[dependencies]`.

```text
workspace/
  my-plc/
    runtime.toml
    io.toml
    trust-lsp.toml
    src/
  libraries/
    my_motion_lib/
      trust-lsp.toml
      src/
```

Use this page when you need the file-system mental model. Use
[First Project](../start/first-project.md) when you want the shortest working
bootstrap.

## Developer Guide

--8<-- "docs/guides/PLC_DEVELOPER_GUIDE.md"
