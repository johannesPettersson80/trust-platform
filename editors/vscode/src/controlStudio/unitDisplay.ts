// Exists as the one place that maps a stored contract unit spelling to the notation a
// student should read (finding V-02/V-07: "degC" appeared verbatim next to "°C" in the
// same course). Every label — axis title, live reading, saved-result card, Signals row —
// must route through this function so the mapping cannot drift between call sites; the
// stored unit string itself is never rewritten.

const TOKEN_DISPLAY_UNITS: ReadonlyArray<readonly [RegExp, string]> = [
  [/\bdegC\b/gu, "°C"],
  [/\bpp\b/gu, "percentage points"],
  [/\*/gu, "·"],
];

/** Converts configuration-safe unit spellings to the compact or plain-language notation
 * a student reads (degC -> °C, pp -> percentage points, * -> ·), including inside a
 * composite unit such as "%/degC", "pp/s" or "%/(degC*s)". Unrecognized units pass
 * through unchanged. */
export function displayEngineeringUnit(unit: string): string {
  const normalized = unit.trim();
  if (!normalized) return normalized;
  return TOKEN_DISPLAY_UNITS.reduce(
    (text, [pattern, replacement]) => text.replace(pattern, replacement),
    normalized
  );
}
