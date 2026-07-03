export interface ServerEndpointSummaryRow {
  label: string;
  value: string;
}

function str(value: unknown): string {
  return value === undefined || value === null ? "" : String(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseArray(raw: unknown, value = ""): unknown[] {
  if (Array.isArray(raw)) {
    return raw;
  }
  if (typeof raw === "string" && raw.trim().startsWith("[")) {
    try {
      const parsed = JSON.parse(raw) as unknown;
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }
  if (typeof value === "string" && value.trim().startsWith("[")) {
    try {
      const parsed = JSON.parse(value) as unknown;
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }
  return [];
}

export function formatExposedGlobals(raw: unknown, value = ""): string {
  const globals = parseArray(raw, value).map((item) => str(item).trim()).filter(Boolean);
  if (globals.length === 0) {
    return "None";
  }
  return `${globals.length} global${globals.length === 1 ? "" : "s"}: ${globals.join(", ")}`;
}

function opcuaEndpoint(params?: Record<string, unknown>): string {
  const listen = str(params?.listen).trim();
  const path = str(params?.endpoint_path).trim();
  if (!listen) {
    return path || "";
  }
  const base = listen.includes("://") ? listen : `opc.tcp://${listen}`;
  if (!path || path === "/") {
    return base;
  }
  const suffix = path.startsWith("/") ? path : `/${path}`;
  return base.endsWith(suffix) ? base : `${base.replace(/\/+$/, "")}${suffix}`;
}

function adsEndpoint(params?: Record<string, unknown>): string {
  const listen = str(params?.listen).trim();
  const amsNetId = str(params?.ams_net_id).trim();
  const adsPort = str(params?.ads_port).trim();
  return [
    listen,
    amsNetId ? `AMS Net ID ${amsNetId}` : "",
    adsPort ? `ADS port ${adsPort}` : "",
  ].filter(Boolean).join(" · ");
}

function connectedClients(live: unknown): string {
  if (!isRecord(live)) {
    return "No live client evidence";
  }
  const raw = live.value;
  let value = raw;
  if (typeof raw === "string" && raw.trim().startsWith("{")) {
    try {
      value = JSON.parse(raw) as unknown;
    } catch {
      value = raw;
    }
  }
  if (!isRecord(value) || value.connected_clients === undefined) {
    return "No live client evidence";
  }
  const count = Number(value.connected_clients);
  if (!Number.isFinite(count)) {
    return "No live client evidence";
  }
  return `${count} client${count === 1 ? "" : "s"} connected`;
}

export function serverEndpointSummaryRows(
  protocol: string,
  params?: Record<string, unknown>,
  live?: unknown
): ServerEndpointSummaryRow[] {
  if (protocol === "opcua") {
    const endpoint = opcuaEndpoint(params);
    return endpoint ? [{ label: "Server endpoint", value: endpoint }] : [];
  }
  if (protocol === "ads_server") {
    const endpoint = adsEndpoint(params);
    return [
      ...(endpoint ? [{ label: "Server endpoint", value: endpoint }] : []),
      { label: "Connected clients", value: connectedClients(live) },
    ];
  }
  return [];
}
