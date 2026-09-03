// Exists because the live-value strip and Signals tab used to hard-code "Controller
// output" for the manipulated variable (finding W-09): Lesson 1 has no controller (the
// bound symbol is the operator's manual demand) and Lesson 8 binds the clamped applied
// actuator reading, so the fixed label taught the opposite of what the loop contract
// actually binds. Deriving the displayed name from the bound runtime symbol keeps every
// tile and series honest about what it shows.

/** Turns a bound runtime symbol (e.g. "P1.AppliedHeaterPower") into a readable label
 * ("Applied heater power") instead of a role-generic placeholder. Falls back to the
 * original text for symbols that carry no distinguishable words (e.g. short role codes
 * like "MV" or "PV"), which are returned unchanged. */
export function readableSignalName(symbol: string): string {
  const trimmed = symbol.trim();
  if (!trimmed) return trimmed;
  const segment = trimmed.split(".").pop() || trimmed;
  const words = segment
    .replace(/([a-z0-9])([A-Z])/gu, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/gu, "$1 $2")
    .replace(/[_-]+/gu, " ")
    .trim()
    .split(/\s+/u)
    .filter(Boolean);
  if (words.length === 0) return trimmed;
  return words.map((word, index) => (index === 0 ? leadWord(word) : word.toLowerCase())).join(" ");
}

/** Keeps a short all-caps role code (MV, PV, SP...) intact instead of forcing "Mv"; any
 * longer or mixed-case word is rendered in ordinary sentence case. */
function leadWord(word: string): string {
  if (word.length <= 3 && word.toUpperCase() === word) return word;
  return word.charAt(0).toUpperCase() + word.slice(1).toLowerCase();
}
