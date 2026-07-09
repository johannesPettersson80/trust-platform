export function adsConnectionIdentityParts(
  connection: Record<string, unknown>
): string[] {
  const amsNetId =
    stringValue(connection.ams_net_id) ||
    stringValue(connection.target_net_id);
  const adsPort = stringValue(connection.ams_port);
  return [
    amsNetId ? `AMS Net ID ${amsNetId}` : "",
    adsPort ? `ADS port ${adsPort}` : "",
  ].filter(Boolean);
}

function stringValue(value: unknown): string {
  return value === undefined || value === null ? "" : String(value).trim();
}
