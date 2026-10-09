// The words for an equipment instance's hardware status (specification 39, §21.3 item 2), shared
// by the builder's inspector and Devices & Connections.
import type { HardwareStatus } from "./equipmentDetails";

const WORDS: Record<HardwareStatus, string> = {
  none: "No hardware connections needed",
  connected: "Connected",
  partly: "Partly connected",
  "not-connected": "Not connected",
  check: "Check connections",
};

export function hardwareWords(status: HardwareStatus): string {
  return WORDS[status];
}
