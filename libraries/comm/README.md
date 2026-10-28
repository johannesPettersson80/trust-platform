# truST Comm Library

The Comm library gives the program and the HMI the state of every communication link the runtime
runs. The program declares a `Comm.Link` block naming its link, for example `modbus_tcp:0`; the
runtime fills its outputs at the start of every scan. The contract is
[specification 42](../../docs/specs/42-communication-links.md); the manual is the
[Comm library guide](../../docs/guides/COMM_LIBRARY_GUIDE.md).

The public namespace contains `Comm.Link`, with its alarm `LinkFailed`, and the enum
`Comm.LinkState`. The library ships the link faceplate and its pair under `hmi/`.

Add the package, and the Alarm library it uses, to a project's `trust-lsp.toml`:

```toml
[dependencies]
Alarm = { path = "../../libraries/alarm", version = "0.1.0" }
Comm = { path = "../../libraries/comm", version = "0.1.0" }
```
