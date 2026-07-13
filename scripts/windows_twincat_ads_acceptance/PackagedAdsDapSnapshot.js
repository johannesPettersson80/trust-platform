"use strict";

function selectedAdsSnapshot(body, remoteSymbol) {
  if (body?.schemaVersion !== 1 || !Array.isArray(body.entries)) return undefined;
  const selected = body.entries.filter(
    (candidate) => candidate?.remoteSymbol === remoteSymbol
  );
  if (selected.length !== 1) return undefined;
  const entry = selected[0];
  return {
    schemaVersion: body.schemaVersion,
    scan: body.scan,
    entry: {
      connection: entry.connection,
      name: entry.name,
      remoteSymbol: entry.remoteSymbol,
      value: entry.value,
      valueType: entry.valueType,
      access: entry.access,
      quality: {
        state: entry.quality?.state,
        lastUpdateMs: entry.quality?.lastUpdateMs ?? null,
        detail: entry.quality?.detail ?? null,
      },
    },
  };
}

module.exports = { selectedAdsSnapshot };
