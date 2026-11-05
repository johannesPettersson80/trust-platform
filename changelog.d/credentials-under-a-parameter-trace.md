### Security

- While the language server trace logs each message's parameters (`trust.languageServer.trace` at `verbose`, or
  `trust-lsp.trace.server` at `verbose`, at `compact` or in the `json` format), the extension refuses every command
  that would carry a credential to the language server, before anything is sent: an HMI deployment, a run target's
  connection and Live Values with their runtime token, a simulator journey's session and its cookie, and a new
  simulator's control token. Before, only a simulator journey was refused, and the others reached VS Code's log
  folder. The language server names these credentials in its `initialize` answer (specification 25 §6.4).
