# Agent API as an MCP server

`trust-dev agent serve-mcp` exposes the same methods as
[`trust-dev agent serve`](v1.md) as a Model Context Protocol (MCP) server, so
MCP-capable hosts can drive truST without a custom client. The tool list is a
1:1 projection of the Agent API methods with dots replaced by underscores
(`workspace.read` becomes `workspace_read`).

The binding, its protocol revision negotiation, and its error mapping are
specified in the repository guide
`docs/guides/AGENT_MCP_BINDING_V1.md` (specification 20 §8).

## Host configuration

Point your MCP host at `trust-dev agent serve-mcp` over stdio:

```json
{
  "mcpServers": {
    "trust-dev": {
      "command": "trust-dev",
      "args": ["agent", "serve-mcp", "--project", "./my-plc"]
    }
  }
}
```

`--project` (alias `--bundle`) selects the workspace root; omit it to use the
host's current directory. The server advertises tools only and serves
requests sequentially, in arrival order.
