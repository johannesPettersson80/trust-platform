# Create A New Project

Use this page when you are starting from an empty folder. This page is for the
current shipped bootstrap path, not an idealized future scaffold.

## What Happens Today

`Structured Text: New Project` currently creates:

```text
my-plc/
  trust-lsp.toml
  src/
    Main.st
```

It does **not** create a full runnable project yet.

## Honest Current-State Workflow

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

## Use A Shipped Tutorial If You Want Less Friction

If you want first success faster than empty-folder bootstrapping, use:

- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Tutorial 13: Bootstrap From Zero](../examples/tutorials.md)

## Next

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Debugging And Runtime Panel](../operate/debugging-and-runtime-panel.md)
