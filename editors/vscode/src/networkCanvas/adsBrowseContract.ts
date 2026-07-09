import * as path from "path";

export function buildOfflineAdsImportArgs(
  projectDir: string,
  target: Record<string, unknown>,
  connectionName: string,
  symbols: readonly string[]
): string[] {
  const host = stringField(target, "host", "ip") ?? "";
  const args = [
    "ads",
    "import-symbols",
    "--target",
    host,
    "--connection",
    connectionName,
    "--out",
    path.join(projectDir, "ads.toml"),
    "--gen",
    path.join(projectDir, "src", "generated", "ads_generated.st"),
    "--force",
    "--json",
  ];
  const targetNetId = stringField(target, "target_net_id", "ams_net_id");
  if (targetNetId) {
    args.push("--target-net-id", targetNetId);
  }
  const amsPort = numberField(target, "ams_port");
  if (amsPort) {
    args.push("--ams-port", String(amsPort));
  }
  for (const symbol of symbols) {
    args.push("--include", symbol);
  }
  return args;
}

export function buildOfflineBrowseSymbolsArgs(
  protocol: string,
  target: Record<string, unknown>,
  kind: "symbols" | "nodes" | "channels" = "symbols",
  connectionName?: string,
  projectDir?: string
): string[] {
  const args = [
    "comm",
    "browse-symbols",
    "--protocol",
    protocol,
    "--kind",
    kind,
    "--json",
  ];
  if (projectDir) {
    args.push("--project", projectDir);
  }
  if (Object.keys(target).length > 0) {
    args.push("--target", JSON.stringify(target));
  }
  if (connectionName) {
    args.push("--connection-name", connectionName);
  }
  return args;
}

export function classifyAdsBrowseCommandFailure(message: string): string {
  const detail = message.toLowerCase();
  if (
    [
      "connection refused",
      "host unreachable",
      "network unreachable",
      "target port",
      "wrong plc port",
      "invalid ams port",
      "port disabled",
      "port not connected",
      "ads port not opened",
      "port not registered",
      "port is invalid",
      "port removed",
    ].some((needle) => detail.includes(needle))
  ) {
    return "ads_port_unavailable";
  }
  if (
    [
      "not supported",
      "unsupported",
      "invalid index group",
      "service is not available",
      "unknown command id",
      "unknown ams command",
    ].some((needle) => detail.includes(needle))
  ) {
    return "symbol_upload_unsupported";
  }
  if (detail.includes("no more symbols in cache")) {
    return "empty_symbol_table";
  }
  return "symbol_upload_failed";
}

function stringField(
  value: Record<string, unknown>,
  ...keys: string[]
): string | undefined {
  for (const key of keys) {
    const item = value[key];
    if (typeof item === "string" && item.trim().length > 0) {
      return item.trim();
    }
  }
  return undefined;
}

function numberField(
  value: Record<string, unknown>,
  key: string
): number | undefined {
  const item = value[key];
  return typeof item === "number" && Number.isFinite(item) ? item : undefined;
}
