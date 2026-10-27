// Display helpers of an ADS browse target. Which port a typed text names is the server's answer
// (adsPortCheck.ts); a target's own port is set by the server (`comm.discover` and the browse
// targets of `trust-lsp.devicesTopology`).

/** The AMS Net ID the browse pane shows for a target. */
export function adsTargetNetId(target: Record<string, unknown>): string {
  const netId = target.ams_net_id ?? target.target_net_id;
  return typeof netId === "string" && netId.trim().length > 0
    ? netId.trim()
    : "Unknown AMS Net ID";
}

/** The ADS port the server set on a target, when it set one. */
export function adsTargetPort(target: Record<string, unknown>): number | undefined {
  return typeof target.ams_port === "number" ? target.ams_port : undefined;
}

/** `target` browsed at `port`. */
export function withAdsTargetPort(
  target: Record<string, unknown>,
  port: number
): Record<string, unknown> {
  return { ...target, ams_port: port };
}

/** The ADS ports that answered, as the server answered them (sorted, each once). */
export function respondingAdsPorts(target: Record<string, unknown>): readonly number[] {
  return Array.isArray(target.responding_ads_ports)
    ? target.responding_ads_ports.filter((port): port is number => typeof port === "number")
    : [];
}
