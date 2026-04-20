# Create A New Project

Create a new project from an empty folder.

## What gets created

`Structured Text: New Project` currently creates:

```text
my-plc/
  trust-lsp.toml
  src/
    Main.st
```

It does **not** create a full runnable project yet.

## Setup steps

1. Run `Structured Text: New Project`.
2. Confirm you got `src/Main.st` and `trust-lsp.toml`.
3. Run `Structured Text: Create/Select Configuration`.
4. Add `runtime.toml`.
5. Add `io.toml`.
6. Build and validate before trying to run.

## Minimum Useful Project Shape

```text
my-plc/
  runtime.toml
  io.toml
  trust-lsp.toml
  src/
    Main.st
    configuration.st
```

## Shortcut: start from a tutorial

If you want a faster start than empty-folder bootstrapping, use:

- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Tutorial 13: Bootstrap From Zero](../examples/tutorials.md)

## Next

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Debugging And Runtime Panel](../operate/debugging-and-runtime-panel.md)
