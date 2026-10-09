// Motion in the symbols (specification 39, §14.2, item 5; §5.3, item 3): what moves in the plant
// moves in its symbol, and only while the value that says so is a current observation.

import type { LiveValue } from "../model";

/** How long one turn of an impeller takes. */
export const SPIN_PERIOD_MS = 2000;

/** How long a pipe's flow dashes take to travel one dash and its gap (§6, item 6). */
export const FLOW_PERIOD_MS = 800;

/** Whether a value is a current observation: not unknown, not stale and not merely commanded. */
export function observed(value: LiveValue | undefined): value is LiveValue {
  return (
    value !== undefined && value.staleMs === undefined && (value.state === "confirmed" || value.state === "estimated")
  );
}

/** The delay that starts a turn where it stands at `nowMs`, so a redraw continues it. */
export function turnDelayMs(nowMs: number | undefined): number {
  return Math.floor(nowMs ?? 0) % SPIN_PERIOD_MS;
}
