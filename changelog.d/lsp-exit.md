### Fixed

- `trust-lsp` ends when its client sends `exit`, with status 0 after `shutdown` and 1 without it,
  also while the client keeps the server's input open. It kept running until that input closed,
  so an editor that leaves the pipe open after `exit` left the server behind (specification 14
  §7.9).
