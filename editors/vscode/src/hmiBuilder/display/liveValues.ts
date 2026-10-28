// Live values for a displayed page (specification 39, §11.1, §5.3): what the runtime's value
// stream sends, kept with its state, staleness and the samples trends draw.

import type { DrawFrame, LiveValue, StructuredValue } from "../drawing/model";

/** One value as the stream sends it. */
export interface WireValue {
  presentation?: LiveValue["presentation"];
  value: StructuredValue;
  lowerBounds?: number[];
  /** At the inputs of a link without data: the time since its last data (spec 42, §5 item 4). */
  staleMs?: number;
}

/** A message of the value stream: the expected update period with the stale limit first, then
 * values. */
export interface StreamMessage {
  period_ms?: number;
  /** How long values may go without a new cycle count or a message before they are stale, as the
   * runtime decides it (§5.3 item 2). */
  stale_ms?: number;
  cycle?: number;
  values?: Record<string, WireValue | null>;
  /** Bound paths the running program does not have (§16.3). */
  missing?: string[];
}

export interface LiveValueOptions {
  /** For each path a trend draws, the longest span it shows, in ms. */
  trendSpansMs?: Record<string, number>;
}

/** The most samples a trend path keeps, whatever its span. */
const MAX_SAMPLES = 20_000;
/** Beyond this span a trend path keeps one sample a second. */
const LONG_SPAN_MS = 600_000;

export class LiveValueStore {
  private readonly trendSpansMs: Record<string, number>;
  private readonly values = new Map<string, WireValue | null>();
  private readonly history = new Map<string, Array<[number, number]>>();
  /** The stale limit the runtime sent; until it has, no value is stale by its age. */
  private staleLimitMs: number | undefined;
  private cycle: number | undefined;
  private lastAdvanceMs: number | undefined;
  private lastMessageMs: number | undefined;
  private disconnectedAtMs: number | undefined;

  constructor(options: LiveValueOptions = {}) {
    this.trendSpansMs = options.trendSpansMs ?? {};
  }

  receive(message: StreamMessage, atMs: number): void {
    this.disconnectedAtMs = undefined;
    if (message.stale_ms !== undefined) this.staleLimitMs = message.stale_ms;
    if (message.cycle === undefined || message.values === undefined) return;
    this.lastMessageMs = atMs;
    // An update carries only what changed (§18 phase 5): the missing paths when their list did.
    if (message.missing !== undefined) this.missing = message.missing;
    const advanced = this.cycle === undefined || message.cycle !== this.cycle;
    this.cycle = message.cycle;
    if (advanced) this.lastAdvanceMs = atMs;
    for (const [path, wire] of Object.entries(message.values)) this.values.set(path, wire);
    if (!advanced) return;
    // Each trend path keeps a sample per new cycle, whether its value changed or not.
    for (const path of Object.keys(this.trendSpansMs)) {
      const wire = this.values.get(path);
      if (wire !== undefined) this.sample(path, wire, atMs);
    }
  }

  /** The paths the running program does not have, from the newest update (§16.3). */
  private missing: string[] = [];

  disconnected(atMs: number): void {
    this.disconnectedAtMs = atMs;
  }

  frame(nowMs: number): Pick<DrawFrame, "values" | "history" | "nowMs" | "missing"> {
    const staleMs = this.staleness(nowMs);
    const values: Record<string, LiveValue> = {};
    for (const [path, wire] of this.values) {
      if (wire === null) {
        values[path] = { value: null, state: "unknown" };
        continue;
      }
      // Stale when the stream stalls, or by itself at the inputs of a link without data.
      const age = Math.max(staleMs ?? -1, wire.staleMs ?? -1);
      values[path] = {
        value: wire.value,
        state: "confirmed",
        ...(wire.presentation ? { presentation: wire.presentation } : {}),
        ...(wire.lowerBounds ? { lowerBounds: wire.lowerBounds } : {}),
        ...(age >= 0 ? { staleMs: age } : {}),
      };
    }
    return { values, history: Object.fromEntries(this.history), nowMs, missing: this.missing };
  }

  /** Whether the stream is closed or its updates are stale (§5.3, item 2), as alarms show it. */
  isStale(nowMs: number): boolean {
    return this.disconnectedAtMs !== undefined || this.staleness(nowMs) !== undefined;
  }

  /** The age of the values when they count as stale, otherwise undefined: the stream is closed, or
   * the cycle count has not advanced or no message has arrived for the runtime's limit (§11.1). */
  private staleness(nowMs: number): number | undefined {
    if (this.lastAdvanceMs === undefined || this.lastMessageMs === undefined) return undefined;
    const limit = this.staleLimitMs;
    const stalled = limit !== undefined && (nowMs - this.lastAdvanceMs > limit || nowMs - this.lastMessageMs > limit);
    return this.disconnectedAtMs !== undefined || stalled ? nowMs - this.lastAdvanceMs : undefined;
  }

  /**
   * Takes a trend path's samples from the runtime's historian (§5.7, item 1): those older than
   * the samples the stream already gave, then the span rule as for the stream's own samples.
   */
  seed(path: string, samples: Array<[number, number]>): void {
    const spanMs = this.trendSpansMs[path];
    if (spanMs === undefined) return;
    const known = this.history.get(path) ?? [];
    const first = known[0]?.[0] ?? Number.POSITIVE_INFINITY;
    const older = samples.filter(([time, value]) => Number.isFinite(time) && Number.isFinite(value) && time < first);
    const merged = [...older, ...known];
    const newest = merged[merged.length - 1]?.[0] ?? 0;
    while (merged.length > 1 && (merged[1][0] < newest - spanMs || merged.length > MAX_SAMPLES)) merged.shift();
    this.history.set(path, merged);
  }

  /** A trend now shows `spanMs` of a path (§5.9 item 1): its samples are kept that long. */
  widen(path: string, spanMs: number): void {
    this.trendSpansMs[path] = Math.max(this.trendSpansMs[path] ?? 0, spanMs);
  }

  /** Keeps a new sample of a trend path, inside its span plus the newest older sample, so the
   * trend can find the value at its left edge. Over a long span, one sample a second is kept. */
  private sample(path: string, wire: WireValue | null, atMs: number): void {
    const spanMs = this.trendSpansMs[path];
    if (spanMs === undefined || wire === null) return;
    const value = typeof wire.value === "boolean" ? Number(wire.value) : wire.value;
    if (typeof value !== "number" || !Number.isFinite(value)) return;
    const samples = this.history.get(path) ?? [];
    const last = samples[samples.length - 1];
    if (spanMs > LONG_SPAN_MS && last && Math.floor(last[0] / 1000) === Math.floor(atMs / 1000)) samples.pop();
    samples.push([atMs, value]);
    while (samples.length > 1 && (samples[1][0] < atMs - spanMs || samples.length > MAX_SAMPLES)) samples.shift();
    this.history.set(path, samples);
  }
}

/** The most paths one history request names (§5.7, item 1). */
const HISTORY_PATHS = 16;

/**
 * Fills the trends of a page or popup with their history from the runtime's historian (§5.7,
 * item 1), over the longest span among them. Without the historian, or when the request fails,
 * the trends start empty and fill from the stream.
 */
export async function loadHistory(
  fetchJson: (url: string) => Promise<{ status: number; body: unknown }>,
  spansMs: Record<string, number>,
  store: LiveValueStore,
): Promise<void> {
  const paths = Object.keys(spansMs);
  if (paths.length === 0) return;
  const span = Math.min(86_400, Math.max(1, Math.ceil(Math.max(...Object.values(spansMs)) / 1000)));
  for (let start = 0; start < paths.length; start += HISTORY_PATHS) {
    const chunk = paths.slice(start, start + HISTORY_PATHS);
    const query = chunk.map((path) => `path=${encodeURIComponent(path)}`).join("&");
    try {
      const answer = await fetchJson(`/api/hmi/history?${query}&span=${span}`);
      const body = answer.body as { samples?: Record<string, Array<[number, number]>> } | null;
      if (answer.status !== 200 || !body?.samples) continue;
      for (const path of chunk) store.seed(path, body.samples[path] ?? []);
    } catch {
      // The trends fill from the stream.
    }
  }
}
