"use strict";

const fs = require("fs");
const path = require("path");

function readSelectedSnapshot(snapshotRoot, mapping, remoteSymbol) {
  if (!fs.existsSync(snapshotRoot)) return undefined;
  const snapshots = [];
  for (const entry of fs.readdirSync(snapshotRoot, { withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith(".symbols.json")) continue;
    try {
      const value = JSON.parse(fs.readFileSync(path.join(snapshotRoot, entry.name), "utf8"));
      if (!validEnvelope(value)) return undefined;
      snapshots.push(value);
    } catch (_error) {
      return undefined;
    }
  }
  const selectedRoutes = snapshots.filter(
    (candidate) => candidate.route_name === mapping.connection
  );
  if (selectedRoutes.length !== 1) return undefined;
  const selected = selectedRoutes[0].symbols.filter(
    (candidate) => candidate?.name === remoteSymbol
  );
  return selected.length === 1 && validSymbol(selected[0], mapping.type)
    ? selected[0]
    : undefined;
}

function validEnvelope(value) {
  return (
    value &&
    value.schema_version === 1 &&
    typeof value.route_name === "string" &&
    value.route_name.length > 0 &&
    Array.isArray(value.symbols)
  );
}

function validSymbol(symbol, expectedType) {
  const descriptor = symbol?.data_type;
  const dimensions = descriptor?.dimensions ?? [];
  const validStringLength =
    expectedType !== "STRING" ||
    (Number.isInteger(descriptor?.string_len) &&
      descriptor.string_len >= 0 &&
      descriptor.string_len <= 65_535);
  return Boolean(
    descriptor &&
      typeof descriptor.source_name === "string" &&
      descriptor.source_name.length > 0 &&
      descriptor.iec_type === expectedType &&
      Array.isArray(dimensions) &&
      dimensions.every(
        (item) =>
          Number.isSafeInteger(item?.lower) &&
          Number.isSafeInteger(item?.upper) &&
          item.upper >= item.lower
      ) &&
      validStringLength &&
      isU32(symbol.index_group) &&
      isU32(symbol.index_offset) &&
      Number.isInteger(symbol.byte_size) &&
      symbol.byte_size > 0 &&
      Array.isArray(symbol.flags) &&
      symbol.flags.includes("read")
  );
}

function isU32(value) {
  return Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
}

function structuredTextType(symbol) {
  const descriptor = symbol.data_type;
  let scalar = descriptor.iec_type;
  if (scalar === "STRING") scalar = `STRING[${descriptor.string_len}]`;
  const dimensions = descriptor.dimensions ?? [];
  if (dimensions.length === 0) return scalar;
  const bounds = dimensions
    .map((item) => `${item.lower}..${item.upper}`)
    .join(", ");
  return `ARRAY[${bounds}] OF ${scalar}`;
}

module.exports = { readSelectedSnapshot, structuredTextType };
