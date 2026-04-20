# Maintain An Existing Project

Start with the existing project tree before you edit anything.

## Typical project layout

An inherited truST project usually has this shape:

```text
project/
  runtime.toml
  io.toml
  trust-lsp.toml
  src/
    main.st
    config.st
  hmi/
```

## First steps

1. Open the project without editing anything yet.
2. Inspect the tree and identify `src/`, `runtime.toml`, `io.toml`, and `hmi/`.
3. Run build and validate before touching code.
4. Inspect the runtime UI or HMI.
5. Change one safe line only.
6. Rerun or redeploy.
7. Verify the effect.

## Questions To Answer First

- what does `src/main.st` control?
- where are `%I/%Q` bindings defined?
- which runtime URLs/endpoints are enabled?
- is there an HMI folder?
- which driver/backend is configured?

## Start with these

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Deploy And Rollback](../operate/deploy-rollback.md)
- [Operator Guide](../operate/operator-guide.md)

## Avoid first:

- Do not start with a blank-folder bootstrap if you already have a project.
- Do not change config and logic in the same first edit.
- Do not deploy before you have a clean build/validate pass.

## Next

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Runtime UI And Control](../operate/runtime-ui-and-control.md)
- [Deploy And Rollback](../operate/deploy-rollback.md)
