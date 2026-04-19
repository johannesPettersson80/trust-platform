# Maintain An Existing Project

Use this page when you inherited a project and need to understand it before
changing it.

## Start With A Real Project Tree

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

## Safe First Workflow

1. open the project without editing anything yet
2. inspect the tree and identify `src/`, `runtime.toml`, `io.toml`, and `hmi/`
3. run build and validate before touching code
4. inspect the runtime or HMI surface
5. change one safe line only
6. rerun or redeploy
7. verify the effect

## Questions To Answer First

- what does `src/main.st` control?
- where are `%I/%Q` bindings defined?
- which runtime URLs/endpoints are enabled?
- is there an HMI folder?
- which driver/backend is configured?

## Good First Pages

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Deploy And Rollback](../operate/deploy-rollback.md)
- [Operator Guide](../operate/operator-guide.md)

## Do Not Start Here

- do not start with a blank-folder bootstrap if you already have a project
- do not change config and logic in the same first edit
- do not deploy before you have a clean build/validate pass

## Next

- [Project Layout](../develop/project-layout.md)
- [Build, Validate, Test](../operate/build-validate-test.md)
- [Runtime UI And Control](../operate/runtime-ui-and-control.md)
- [Deploy And Rollback](../operate/deploy-rollback.md)
