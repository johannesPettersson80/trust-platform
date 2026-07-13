export const ADS_SERVICE_CHECK_FAILED_COPY =
  "The ADS device was found, but its services could not be checked. Make sure the device is running and your firewall allows truST on this network, then try again.";

export const ADS_BROWSE_FAILED_COPY =
  "The selected ADS service could not return variables. Make sure it is running, then try again.";

/** Preserve only errors already written as safe, actionable product copy. */
export function adsServiceProbeVisibleError(raw: string): string {
  const detail = raw.trim();
  if (ADS_SERVICE_SAFE_MESSAGES.has(detail)) {
    return detail;
  }
  return ADS_SERVICE_CHECK_FAILED_COPY;
}

const ADS_SERVICE_SAFE_MESSAGES = new Set([
  "The selected runtime already owns an active ADS connection. Read-only service checks are paused to protect live PLC I/O. Stop that ADS connection before retrying.",
  "truST could not verify whether the selected runtime owns an ADS connection, so read-only service checks were paused to protect PLC I/O. Reconnect or update that runtime, then retry.",
  "The selected discovery runtime is no longer reachable. Reconnect it and discover ADS devices again.",
  "Check canceled because another ADS device check started.",
]);

export function adsTechnicalDetail(raw: string | undefined): string | undefined {
  const detail = raw?.trim();
  return detail ? detail : undefined;
}
