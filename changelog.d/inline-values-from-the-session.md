### Fixed

- Inline values in a VS Code debug session show the program's values on every platform without a setting: the language
  server takes them from the simulator that the launch started for the document's workspace folder, with the endpoint
  and token it named for that folder. The examples' `trust-lsp.toml` files no longer name `/tmp/trust-debug.sock`,
  where no debug session listened.
