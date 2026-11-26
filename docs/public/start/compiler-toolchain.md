# The Compiler Toolchain

`trust-runtime build` compiles a project into its program image with truST's own compiler
toolchain: the Rust compiler of the release, the libraries every program links, and what a
project with Rust POUs builds with. You never install Rust for it. truST installs its toolchain
once per version into its own data folder, and never uses or touches a Rust of your own.

The steps below install it from VS Code, from a terminal, and on a computer without internet.
Their outputs come from truST 0.44.1 on a Linux x86-64 computer.

## What you need

- truST: the VS Code extension, which carries the toolchain, or `trust-runtime` from a release
  archive.
- About 680 MB of disk for the installed toolchain, and 225 MB more while a downloaded bundle
  installs.

## In VS Code

The extension carries the toolchain of your computer in its package, so you need not do
anything: the first build that needs it, a **Start** or a **Compile**, installs it with no
download and shows its progress. To install it before that, run **truST: Install Toolchain**
from the Command Palette. When an install fails, the extension offers
**truST: Install Toolchain from File**, which installs a bundle file that you choose
([On a computer without internet](#on-a-computer-without-internet)).

## From a terminal

1. See whether it is installed:

    ```text
    trust-runtime toolchain status
    ```

    On a computer without it, this prints:

    ```text
    the Rust toolchain: the truST toolchain 0.44.1 is not installed: `trust-runtime toolchain install` installs it, from the bundle beside the tools or downloaded (about 195 to 225 MB, once)
    ```

2. Install it:

    ```text
    trust-runtime toolchain install
    ```

    `trust-runtime` from a release archive carries no bundle, so it downloads the bundle of your
    computer from the release, checks it and installs it, here for a user named `me`:

    ```text
    Downloading the truST toolchain 0.44.1 (about 195 to 225 MB, once)...
      23 of 222 MB
      46 of 222 MB
      ...
      222 of 222 MB
    Checking the toolchain against its manifest...
    The truST toolchain is installed in /home/me/.local/share/trust/toolchains/0.44.1
    ```

3. Check it again with `trust-runtime toolchain status`:

    ```text
    The truST toolchain 0.44.1 is installed in /home/me/.local/share/trust/toolchains/0.44.1 (rustc 1.99.0)
    ```

An install that finds the toolchain in place says so and changes nothing, and two installs at
once take turns.

## On a computer without internet

1. On a computer with internet, open the release of your truST version, such as
   <https://github.com/johannesPettersson80/trust-platform/releases/tag/v0.44.1>, and download
   the bundle of the offline computer: `trust-toolchain-linux-x64.tar.gz`,
   `trust-toolchain-linux-arm64.tar.gz` (a Raspberry Pi), `trust-toolchain-darwin-x64.tar.gz`,
   `trust-toolchain-darwin-arm64.tar.gz` or `trust-toolchain-win32-x64.tar.gz`. Check its SHA-256
   against its line in the release's `SHA256SUMS`.
2. Copy it to the offline computer and install it from the file:

    ```text
    trust-runtime toolchain install --from trust-toolchain-linux-x64.tar.gz
    ```

    It installs without the download:

    ```text
    Checking the toolchain against its manifest...
    The truST toolchain is installed in /home/me/.local/share/trust/toolchains/0.44.1
    ```

    In VS Code, **truST: Install Toolchain from File** does the same with the file you choose.

## What happened

**Where it lies.** Each truST version has its own toolchain, in truST's data folder of your user,
which the extension and `trust-runtime` share, so that one install serves both:

| Computer | Folder |
| --- | --- |
| Linux | `$XDG_DATA_HOME/trust/toolchains/<version>`, else `~/.local/share/trust/toolchains/<version>` |
| macOS | `~/Library/Application Support/trust/toolchains/<version>` |
| Windows | `%LOCALAPPDATA%\trust\toolchains\<version>` |

It needs no administrator rights. The bundle file of 0.44.1 for Linux x86-64 is 233,437,888
bytes, the "222 MB" of the download, and the installed toolchain takes 680 MB.

**What is checked.** A download is checked against the release's `SHA256SUMS`. Every install
unpacks the bundle beside its folder, checks each file's size and SHA-256 against the bundle's
manifest, and the manifest against your truST version and computer, and only then moves it into
place. A stopped or failed install therefore leaves no toolchain that the tools would use.

**When a build installs it.** `trust-runtime build`, `play`, `run` of a project without a program
image, and `fleet start` install the toolchain when they find none. In a terminal the build asks
first, naming where the bundle comes from, its size and its folder. In a script or CI, without a
terminal, it fails and advises `trust-runtime toolchain install`, unless `--install-toolchain` on
`build`, `play` or `run` gives the consent:

```text
trust-runtime build --project . --install-toolchain
```

**For scripts.** `trust-runtime toolchain status --json` prints one object:

```text
{"folder":"/home/me/.local/share/trust/toolchains/0.44.1","installed":true,"source":"installed","trust":"0.44.1"}
```

`trust-runtime toolchain install --json` writes one object per line: `{"event": "download",
"bytes": <n>, "total": <n>}` while it downloads, `{"event": "verify"}`, and then
`{"event": "installed", "folder": <path>}` or `{"event": "failed", "error": <text>}`.

**A development toolchain.** The environment variable `TRUST_TOOLCHAIN` names the toolchain folder
of a development checkout (`scripts/native/build_toolchain_libraries.sh`), which the tools then
use, with nothing downloaded; `status --json` gives its `source` as `TRUST_TOOLCHAIN`.
`trust-runtime toolchain bundle <folder> <archive>` writes a bundle from such a folder, as the
release does for each computer ([Install From Source](install-from-source.md)).

## Related

- [Install truST](installation.md)
- [Install From Source](install-from-source.md)
- [Offline Install](../operate/offline-install.md)
- [`trust-runtime` reference](../reference/cli/trust-runtime.md#toolchain)
- Specification 44, "The toolchain bundle" (§4.6)
