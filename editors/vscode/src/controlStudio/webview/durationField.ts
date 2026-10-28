import { formatEngineeringNumber } from "../chartContract";

/**
 * A number of seconds as plain decimal text. The display formats values below 1 ms as exponents (`5.00e-4`), which the engine's duration
 * reader refuses, so a duration the page sends never carries one (specification 35, B9.9).
 */
function plainSeconds(seconds: number): string {
  const text = formatEngineeringNumber(seconds);
  if (!/e/i.test(text)) return text;
  return seconds.toFixed(12).replace(/0+$/, "").replace(/\.$/, "");
}

/**
 * Shows a number of seconds in its largest fitting unit: seconds below 2 min, minutes below 2 h, then hours and minutes. This is a display
 * only: a number rounds to the display's precision (`formatEngineeringNumber`), so the text need not read back as the same number, and the editor
 * sends the number back while the text is unchanged (`compactEditor.ts`, `originals`). Only the Requirements editor uses it, for the settling
 * time and the validation duration, which the engine answers as numbers; a duration the engine answers as text, such as the Limits' maximum
 * duration (`durationWords`) and an analysis input, is shown as the engine wrote it (`duration_words`, B9.9), and a text is never formatted here.
 */
export function formatDurationSeconds(seconds: number): string {
  if (seconds < 120) return `${plainSeconds(seconds)} s`;
  if (seconds < 7_200) {
    const minutes = Math.floor(seconds / 60);
    const rest = seconds - minutes * 60;
    return rest === 0 ? `${minutes} min` : `${minutes} min ${plainSeconds(rest)} s`;
  }
  const hours = Math.floor(seconds / 3_600);
  const restOfHour = seconds - hours * 3_600;
  const minutes = Math.floor(restOfHour / 60);
  const rest = restOfHour - minutes * 60;
  return [`${hours} h`, minutes ? `${minutes} min` : "", rest ? `${plainSeconds(rest)} s` : ""].filter(Boolean).join(" ");
}
