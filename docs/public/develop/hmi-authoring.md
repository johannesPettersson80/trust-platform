# HMI Authoring

Use this page when you are creating or changing the project-owned `hmi/`
directory. It is the canonical authoring page for descriptor files, process SVG
bindings, write policy, and AI-assisted HMI tooling.

The guide below teaches:

- HMI folder layout
- widget/schema expectations
- the path from project files to runtime-hosted UI
- validation and preview order before accepting changes

After reading it, you should be able to scaffold `hmi/`, bind live values to a
page, keep writes disabled by default, and preview the result from the same
project. If you only need to operate an already-running HMI, start with
[HMI And Web UI](../operate/hmi-and-web-ui.md).

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

## Related

- [HMI And Web UI](../operate/hmi-and-web-ui.md)
- [Program In Browser IDE](../start/program-in-browser.md)
- [HMI examples](../examples/hmi.md)
