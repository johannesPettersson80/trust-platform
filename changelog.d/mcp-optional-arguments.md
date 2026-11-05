### Fixed

- The HMI authoring MCP server (`trust-lsp hmi-agent`) refused a tool call without `arguments`,
  which MCP allows for a tool that takes none. It now runs the call with no arguments
  (specification 43 §3.1).
