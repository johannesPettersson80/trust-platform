# Package Registry

## Use it for

- reusable libraries
- shared PLC packages across projects
- internal package distribution

## Local registry lifecycle

| Step | Command |
| --- | --- |
| describe the contract | `trust-runtime registry profile` |
| initialize a registry root | `trust-runtime registry init --root ./registry` |
| publish a project | `trust-runtime registry publish --registry ./registry --project ./my-lib --version 0.1.0` |
| list packages | `trust-runtime registry list --registry ./registry` |
| download a package | `trust-runtime registry download --registry ./registry --name MyLib --version 0.1.0 --output ./vendor/MyLib` |
| verify digests | `trust-runtime registry verify --registry ./registry --name MyLib --version 0.1.0` |

## Worked example

1. Create a reusable package at `./libraries/my_motion_lib`.
2. Give it its own `trust-lsp.toml` and `src/`.
3. Publish it:

```bash
trust-runtime registry init --root ./registry
trust-runtime registry publish --registry ./registry --project ./libraries/my_motion_lib --version 0.1.0
```

4. Consume it from another project by adding a dependency entry or downloading it into a vendor path.

## Local registry vs direct path dependency

| Approach | Best for |
| --- | --- |
| `[dependencies]` path entry | same repo or adjacent workspace |
| local registry | versioned internal distribution, reproducible installs |

## Current surface

The shipped workflow is centered on `trust-runtime registry ...` subcommands and
`trust-lsp.toml` dependencies.

## Verification

After publishing a package, run `trust-runtime registry list` to confirm the
package/version is discoverable, then run `trust-runtime registry verify` for the
same name and version before consuming it from another project.

## Related pages

- [Project Layout](project-layout.md)
- [Libraries](libraries/index.md)
- [trust-runtime CLI](../reference/cli/trust-runtime.md)
