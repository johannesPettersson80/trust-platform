# FAQ

Quick answers to common product, runtime, and workflow questions. For failures,
use [Troubleshooting](troubleshooting.md).

## Is truST safety-rated?

No. Treat truST as an engineering/runtime platform, not a safety-rated PLC.
Read [Safety And Commissioning](operate/safety-and-commissioning.md) before live
plant use.

## Can I use truST commercially?

See the dual-license terms in [About](about.md) and the repository license
files. Legal certainty belongs with your organization’s license review.

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

See [Hardware Compatibility](reference/hardware-compatibility.md). truST runs
on Linux hosts including Raspberry Pi and other ARM64 systems. Windows and
macOS are commonly used for tooling.

## How is truST different from other PLC runtimes or vendor IDEs?

truST combines:

- IEC 61131-3 Structured Text support
- a browser-based operator HMI at `/hmi`
- an agent-ready JSON-RPC API
- open-source licensing under MIT and Apache-2.0

See:

- [About](about.md)
- [What Is truST?](concepts/index.md)
- [Migration While Programming](migrate/index.md)

## Where do I start if I inherited a project?

Go to [Maintain An Existing Project](start/maintain-an-existing-project.md).

## Where do I start if I only have an HMI URL?

Go to [Operate In Browser HMI](start/operate-in-browser.md).

## Where do I start if I want CI or agent automation?

Go to [Automate With CLI / CI / agents](start/automate-with-cli.md).
