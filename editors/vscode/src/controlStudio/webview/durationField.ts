import { formatEngineeringNumber } from "../chartContract";

/** Seconds per accepted unit word. Only whole words are accepted, so "m" alone (metres) is not a duration. */
const UNIT_SECONDS: Readonly<Record<string, number>> = {
  h: 3_600, hr: 3_600, hrs: 3_600, hour: 3_600, hours: 3_600,
  min: 60, mins: 60, minute: 60, minutes: 60,
  s: 1, sec: 1, secs: 1, second: 1, seconds: 1,
};

/** Parses "90 s", "15 min", "2 h", "1 h 30 min" or a plain number of seconds. Returns undefined for anything else. */
export function parseDurationSeconds(text: string): number | undefined {
  const source = text.trim().toLowerCase();
  if (/^\d+(?:\.\d+)?$/.test(source)) return Number(source);
  const pair = /(\d+(?:\.\d+)?)\s*([a-z]+)\s*/y;
  let total = 0;
  let consumed = 0;
  let match: RegExpExecArray | null;
  while ((match = pair.exec(source)) !== null) {
    const factor = UNIT_SECONDS[match[2]];
    if (factor === undefined) return undefined;
    total += Number(match[1]) * factor;
    consumed = pair.lastIndex;
  }
  return consumed > 0 && consumed === source.length ? total : undefined;
}

/**
 * Shows a duration in its largest fitting unit: seconds below 2 min, minutes below 2 h, then hours and minutes.
 * Any remainder is written in the next smaller unit, so the text parses back to the same number of seconds.
 */
export function formatDurationSeconds(seconds: number): string {
  if (seconds < 120) return `${formatEngineeringNumber(seconds)} s`;
  if (seconds < 7_200) {
    const minutes = Math.floor(seconds / 60);
    const rest = seconds - minutes * 60;
    return rest === 0 ? `${minutes} min` : `${minutes} min ${formatEngineeringNumber(rest)} s`;
  }
  const hours = Math.floor(seconds / 3_600);
  const restOfHour = seconds - hours * 3_600;
  const minutes = Math.floor(restOfHour / 60);
  const rest = restOfHour - minutes * 60;
  return [`${hours} h`, minutes ? `${minutes} min` : "", rest ? `${formatEngineeringNumber(rest)} s` : ""].filter(Boolean).join(" ");
}
