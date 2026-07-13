"use strict";

function readTomlAssignment(line) {
  const quoted = line.match(
    /^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*("(?:[^"\\]|\\.)*")\s*(?:#.*)?$/
  );
  if (quoted) {
    try {
      return { key: quoted[1], value: JSON.parse(quoted[2]) };
    } catch (_error) {
      return undefined;
    }
  }
  const integer = line.match(
    /^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*([0-9]+)\s*(?:#.*)?$/
  );
  if (integer) {
    const value = Number(integer[2]);
    return Number.isSafeInteger(value)
      ? { key: integer[1], value }
      : undefined;
  }
  const boolean = line.match(
    /^\s*([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*(true|false)\s*(?:#.*)?$/
  );
  return boolean
    ? { key: boolean[1], value: boolean[2] === "true" }
    : undefined;
}

module.exports = { readTomlAssignment };
