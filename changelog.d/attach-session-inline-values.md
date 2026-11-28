### Added

- Inline values now show the runtime's values in a debug session that attaches to a running runtime, not only in a
  session that launched the simulator. The extension tells the language server which endpoint the session attached
  to; the language server finds the token of the workspace folder's own runtime itself, as `trust-runtime ctl
  --project` does, and sends it only to that runtime, so no token crosses between the editor and the language server.
