### Added

- `trust-dev agent serve-mcp` exposes the Agent API v1 as an MCP server over
  stdio: the same 21 methods, one MCP tool each, with the Agent API's
  contract-stable error codes preserved in tool results (specification 20 §8,
  `docs/guides/AGENT_MCP_BINDING_V1.md`).
