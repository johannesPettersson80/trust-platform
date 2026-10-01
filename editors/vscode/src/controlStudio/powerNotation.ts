const SUPERSCRIPT_DIGITS = "⁰¹²³⁴⁵⁶⁷⁸⁹";

/** A power of s as a student reads it: s, s², s¹⁰. */
export function powerOfS(power: number): string {
  if (power === 1) return "s";
  return `s${String(power).replace(/\d/g, digit => SUPERSCRIPT_DIGITS[Number(digit)])}`;
}
