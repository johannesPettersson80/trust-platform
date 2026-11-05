### Added

- `trust-dev agent serve-mcp` serves the Agent API v1 as MCP tools over the
  platform's shared MCP connection layer (`trust_lsp::mcp`): a 1:1 tool
  mapping of the contract's methods (dots become underscores), advisory
  generated input schemas, contract errors as error tool results whose text
  carries the stable `{code, message, data}` object, sequential serving with
  one shared harness session per instance, and no configuration beyond the
  `--project`/`--bundle` flag. The binding is specified in specification 20
  §8 and `docs/guides/AGENT_MCP_BINDING_V1.md`.
