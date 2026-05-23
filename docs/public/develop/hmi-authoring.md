# HMI Authoring

Create or change the project-owned `hmi/` directory: descriptor files, process
SVG bindings, write policy, validation, preview, and AI-assisted HMI tooling.

For operating an already-running HMI, use [HMI And Web UI](../operate/hmi-and-web-ui.md).

## HMI Directory Workflow

![Browser HMI overview](../assets/images/browser/hmi-home.png)

*Figure:* A rendered HMI page from the shipped tutorial. Read the workflow
below while comparing the browser view with the `hmi/` files that define it.

--8<-- "docs/guides/HMI_DIRECTORY_WORKFLOW.md:3"

## What Success Looks Like

- `hmi/` exists in the project and contains the descriptors, assets, and policy
  files the runtime will serve.
- Preview shows the expected widgets with live values before any write-capable
  control is enabled.
- 3D HMI pages use `kind = "scene3d"` page descriptors with topology sources
  and compiled static view payloads under `hmi/views/`, so generated view
  payloads are not discovered as normal pages.
- 3D topology sources describe components, ports, connections, and component
  signals; the compiler emits deterministic `.view.toml` output with a source
  hash header for drift checks.
- 3D operator writes are declared as topology `[[interactions]]` and compiled
  into node-level `hmi.write` descriptors; runtime execution still goes through
  the normal HMI write allowlist, Engineer role policy, and audit path.

## Related

- [HMI directory reference](../reference/config/hmi-directory.md)
- [HMI And Web UI](../operate/hmi-and-web-ui.md)
- [Program In Browser IDE](../start/program-in-browser.md)
- [HMI examples](../examples/hmi.md)
