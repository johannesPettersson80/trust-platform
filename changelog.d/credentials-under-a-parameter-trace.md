### Security

- While the language server trace logs each message's parameters (`trust.languageServer.trace` at `verbose`, or
  `trust-lsp.trace.server` at `verbose`, at `compact` or in the `json` format), the extension refuses every command
  that would carry a credential to the language server, before anything is sent, and says so once a minute with the
  setting to change:
  - an HMI deployment, a run target's connection and Live Values with their runtime token;
  - Devices & Connections with its runtime token or a protocol form's password;
  - the HMI users editor with the project's `runtime.toml`;
  - a simulator journey's session and its cookie;
  - a new simulator's control token. A simulator launch then stops instead of starting without a token.
  Before, only a simulator journey was refused, and the others reached VS Code's log folder. A language-model tool
  never sends or receives such a command. The language server names these credentials in its `initialize` answer
  (specification 25 §6.4).
