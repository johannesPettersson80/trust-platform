### Fixed

- A change to `trust-lsp.toml` takes effect at once in the editor: a library, library folder or dependency that the file
  no longer names leaves the project, so a name only it declared is reported again, and it comes back with the entry.
  It used to keep its declarations until the language server restarted. A document of it that is open stays while it is
  open.
