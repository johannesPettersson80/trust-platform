### Fixed

- A `.st` file opened from outside the project's folders leaves the language server's project when it is closed, so
  the project is the one that `trust-lsp check` and the build see. Its declarations counted until the server
  restarted, so a `PROGRAM Main` in it showed a false duplicate declaration on the project's own `Main`
  (specification 14 §7.2.1).
- Closing a file that changes the project, because the file leaves it or unsaved edits are discarded, refreshes the
  diagnostics of the other open files. VS Code kept their earlier diagnostics until each file's next edit.
