// Value and age formatting for the drawing.

import { noteCut } from "./cuts";
import { textWidth } from "./measure";
import type { HmiObjectModel, StructuredValue } from "./model";
import { language, say } from "./speech";

export { textWidth } from "./measure";

/** A value as the object shows it: numbers with the object's decimals, BOOL as IEC TRUE/FALSE. */
export function formatValue(value: StructuredValue | undefined, object: Pick<HmiObjectModel, "decimals">): string {
  if (value === null || value === undefined) return "?";
  if (typeof value === "boolean") return value ? "TRUE" : "FALSE";
  if (typeof value === "string") return durationWords(value, object.decimals) ?? value;
  if (typeof value !== "number" || !Number.isFinite(value)) return String(value);
  if (object.decimals !== undefined) return localNumber(value, object.decimals, object.decimals);
  // Without decimals: up to three, trailing zeros dropped.
  return localNumber(Number(value.toFixed(3)), 0, 3);
}

/** An IEC duration literal as the runtime sends it (§11.1), for example `T#1h2m3s500ms`. */
const DURATION = /^T#(-)?(?:(\d+)d)?(?:(\d+)h)?(?:(\d+)m(?!s))?(?:(\d+)s)?(?:(\d+)ms)?(?:(\d+)us)?(?:(\d+)ns)?$/;

/** A duration in words of its parts, for example `1 h 2 min 3.5 s` (§5.3 item 8); with `decimals`
 * its seconds are rounded to them, and a minute they round up to is carried (`2 min 54 s`). */
function durationWords(literal: string, decimals?: number): string | undefined {
  const match = DURATION.exec(literal);
  if (!match || literal === "T#" || literal === "T#-") return undefined;
  const [, minus, days, hours, minutes, seconds, millis, micros, nanos] = match;
  if (decimals !== undefined) {
    const scale = 10 ** decimals;
    const total =
      Math.round(
        (Number(days ?? 0) * 86400 +
          Number(hours ?? 0) * 3600 +
          Number(minutes ?? 0) * 60 +
          Number(seconds ?? 0) +
          Number(millis ?? 0) / 1e3 +
          Number(micros ?? 0) / 1e6 +
          Number(nanos ?? 0) / 1e9) *
          scale,
      ) / scale;
    const whole = Math.floor(total);
    const rounded: string[] = [];
    if (whole >= 86400) rounded.push(`${Math.floor(whole / 86400)} d`);
    if (whole % 86400 >= 3600) rounded.push(`${Math.floor((whole % 86400) / 3600)} h`);
    if (whole % 3600 >= 60) rounded.push(`${Math.floor((whole % 3600) / 60)} min`);
    const rest = total - Math.floor(total / 60) * 60;
    if (rest > 0 || rounded.length === 0) rounded.push(`${localNumber(rest, decimals, decimals)} s`);
    return `${minus ?? ""}${rounded.join(" ")}`;
  }
  const parts: string[] = [];
  if (days) parts.push(`${Number(days)} d`);
  if (hours) parts.push(`${Number(hours)} h`);
  if (minutes) parts.push(`${Number(minutes)} min`);
  const second =
    Number(seconds ?? 0) + Number(millis ?? 0) / 1e3 + Number(micros ?? 0) / 1e6 + Number(nanos ?? 0) / 1e9;
  if (second > 0 || parts.length === 0) parts.push(`${localNumber(Number(second.toFixed(3)), 0, 3)} s`);
  return `${minus ?? ""}${parts.join(" ")}`;
}

const NUMBER_FORMATS = new Map<string, Intl.NumberFormat>();

/** A number in the language's own way, with its decimal sign and no grouping (§14.5). */
function localNumber(value: number, fewest: number, most: number): string {
  const spoken = language();
  if (spoken === "en") return fewest === most ? value.toFixed(fewest) : value.toString();
  const key = `${spoken}/${fewest}/${most}`;
  let format = NUMBER_FORMATS.get(key);
  if (!format) {
    format = new Intl.NumberFormat(spoken, {
      minimumFractionDigits: fewest,
      maximumFractionDigits: most,
      useGrouping: false,
    });
    NUMBER_FORMATS.set(key, format);
  }
  return format.format(value);
}

/** Text cut with "…" to fit a width at a size and weight; text that fits stays whole. */
export function fitWidth(content: string, width: number, size: number, weight = 400): string {
  if (textWidth(content, size, weight) <= width) return content;
  noteCut(content);
  const letters = Array.from(content);
  while (letters.length > 0 && textWidth(`${letters.join("")}…`, size, weight) > width) letters.pop();
  return `${letters.join("").trimEnd()}…`;
}

/** Text cut to a width at a fixed character width, ending in an ellipsis when it does not fit. */

export function fitText(content: string, width: number, charWidth: number): string {
  const fits = Math.max(1, Math.floor(width / charWidth));
  return content.length > fits ? `${content.slice(0, Math.max(0, fits - 1))}…` : content;
}

/** Local time of day with seconds, as alarms and records show it. */
export function timeOfDay(ms: number): string {
  const time = new Date(ms);
  const two = (value: number): string => String(value).padStart(2, "0");
  return `${two(time.getHours())}:${two(time.getMinutes())}:${two(time.getSeconds())}`;
}

/** The age of a stale value: seconds below a minute, then minutes, then hours. */
export function formatAge(milliseconds: number): string {
  const seconds = Math.max(0, Math.round(milliseconds / 1000));
  if (seconds < 60) return say("{seconds} s", { seconds });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return say("{minutes} min", { minutes });
  return say("{hours} h", { hours: Math.floor(minutes / 60) });
}
