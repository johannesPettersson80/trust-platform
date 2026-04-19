# FAQ

Use this page when you have a plain-language question that is not a failure
symptom.

## Is truST safety-rated?

No. Treat truST as an engineering/runtime platform, not a safety-rated PLC.
Read [Safety And Commissioning](operate/safety-and-commissioning.md) before live
plant use.

## Can I use truST commercially?

See the dual-license terms in [About](about.md) and the repository license
files. If you need legal certainty for a deployment, review those license texts
with your organization.

## Is truST VS Code only?

No.

- VS Code is the primary engineering workflow
- Browser IDE exists at `/ide`
- Browser HMI exists at `/hmi`
- CLI, harness, and agent workflows also exist

Start here:

- [Program In VS Code](start/program-in-vscode.md)
- [Program In Browser IDE](start/program-in-browser.md)
- [Automate With CLI / CI / agents](start/automate-with-cli.md)

## Is Browser IDE the same thing as GitHub Pages docs?

No. The docs site is documentation only. Browser IDE and HMI are served by a
running `trust-runtime`.

## Can I use truST without internet access?

Yes, but you need an offline install path for binaries, dependencies, and any
site-specific assets. See [Offline Install](operate/offline-install.md).

## What hardware does it run on?

See [Hardware Compatibility](reference/hardware-compatibility.md). The short
answer is that Linux and Raspberry Pi / edge Linux are the most natural runtime
targets today, while Windows/macOS are commonly used for tooling.

## How is truST different from OpenPLC or vendor IDEs?

truST focuses on a docs-first, workflow-first ST toolchain with:

- strong editor tooling
- explicit runtime/config/reference docs
- browser surfaces
- agent and deterministic harness surfaces

See:

- [About](about.md)
- [Concepts](concepts/index.md)
- [Interoperability](develop/interoperability/index.md)

## Where do I start if I inherited a project?

Go to [Maintain An Existing Project](start/maintain-an-existing-project.md).

## Where do I start if I only have an HMI URL?

Go to [Operate In Browser HMI](start/operate-in-browser.md).

## Where do I start if I want CI or agent automation?

Go to [Automate With CLI / CI / agents](start/automate-with-cli.md).
