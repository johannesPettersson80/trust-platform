# Installation On Windows, macOS, And Linux

Install the truST editor extension and runtime bundle on Windows, macOS, or
Linux.

## Step 1: Install VS Code

Download VS Code from <https://code.visualstudio.com/> and install the stable
build for your operating system. If you already use VS Code for PLC or
industrial projects, keep that install and move to step 2.

## Step 2: Install truST LSP From The Marketplace

Open the Visual Studio Marketplace listing for truST LSP:

<https://marketplace.visualstudio.com/items?itemName=trust-platform.trust-lsp>

![truST LSP Marketplace listing](../assets/images/install/marketplace-listing.png)

*Figure:* The Marketplace listing for `truST LSP`. Install the extension from
this page or directly from VS Code.

## Step 3: Verify The Extension In VS Code

Open VS Code, press `Ctrl+Shift+P` (`Cmd+Shift+P` on macOS), and type
`truST:`. If the extension is healthy, the truST commands appear immediately in
the Command Palette.

The extension carries the compiler toolchain that builds your programs. The
first build installs it, offline too; you do not install Rust.

![VS Code Command Palette with the truST commands](../assets/images/install/command-palette-trust.png)

*Figure:* `truST:` commands visible in the Command Palette. This is the fastest
proof that the editor-side install worked.

## Step 4: Download Runtime Binaries

Download the current runtime bundle from GitHub Releases:

<https://github.com/johannesPettersson80/trust-platform/releases/latest>

Choose the package that matches your machine:

- `trust-runtime-linux-x64.tar.gz`
- `trust-runtime-linux-arm64.tar.gz`
- `trust-runtime-darwin-x64.tar.gz`
- `trust-runtime-darwin-arm64.tar.gz`
- `trust-runtime-win32-x64.zip`

The Linux packages run on every Linux with glibc 2.28 or newer: Debian 10,
Ubuntu 18.10, Red Hat Enterprise Linux 8 and their later releases. The first
line of `ldd --version` shows your glibc version.

Use [Install On Target](../operate/install-on-target.md) when you need the
runtime to survive reboot on a Raspberry Pi or another Linux host.

![Runtime binary version and help checks](../assets/images/terminal/install-version-checks.gif)

*Figure:* After download and install, `trust-runtime --version` and
`trust-runtime --help` should respond locally. This is the fastest terminal
check that the runtime bundle landed on your machine correctly.

## If It Did Not Work

### No `truST:` commands appear

- restart VS Code once
- confirm the extension is enabled for this workspace
- reinstall the extension from the Marketplace listing

### The runtime download is missing for your platform

- check the latest GitHub release again
- if you need an unreleased build, use [Install From Source](install-from-source.md)

### A Linux binary stops with ``version `GLIBC_2.43' not found``

The Linux x64 packages of 0.35.0 to 0.41.1 needed glibc 2.43, so their
`trust-runtime`, `trust-dev` and debugger did not start on Ubuntu 24.04,
Debian 13 or older Linux. Their language server needed glibc 2.38. Install
0.41.2 or newer, whose Linux packages run with glibc 2.28 or newer. Update the
extension from the Marketplace too: it carries its own copies of these
programs.

## Next

- [Choose Your Workflow](choose-your-workflow.md)
- [Program In VS Code](program-in-vscode.md)
- [Program In Browser IDE](program-in-browser.md)
- [Install On Target](../operate/install-on-target.md)
