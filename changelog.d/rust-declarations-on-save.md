### Added

- The VS Code extension writes a project's generated declarations of its Rust POUs when a `.rs` file
  of the project is saved and once when it opens the project, running `trust-dev rust declarations`
  (specification 48 RU-24), so a crate's changes reach Structured Text without a build; a failure
  is shown with a notification and the command's report in the output channel
  "truST Rust declarations".
