### Changed

- The language server analyzes each project of a workspace folder on its own: every folder that holds a
  `trust-lsp.toml` is a project, a file belongs to the deepest one above it, and a project sees its own files and
  its library folders, never another project's (specification 14 §7.6). In a folder of several projects, such as a
  folder of examples or of PLC projects, one project's `PROGRAM Main` no longer collides with another's, completion
  offers only the project's own names, and each file follows its own project's configuration. The language rules
  still follow the workspace folders' `vendor_profile`.
