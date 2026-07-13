"use strict";

const fs = require("fs");
const path = require("path");
const { readSelectedPointProof } = require("./PackagedAdsTomlProof");
const {
  readSelectedSnapshot,
  structuredTextType,
} = require("./PackagedAdsSnapshotProof");
const { readGeneratedProof } = require("./PackagedAdsGeneratedProof");

function readProjectAdsImport(projectRoot, remoteSymbol, expectedRoute) {
  const adsToml = readIfPresent(path.join(projectRoot, "ads.toml"));
  const runtimeToml = readIfPresent(path.join(projectRoot, "runtime.toml"));
  const selectedPoint = readSelectedPointProof(
    adsToml,
    remoteSymbol,
    expectedRoute
  );
  const mapping = selectedPoint.mapping;
  const snapshot = mapping
    ? readSelectedSnapshot(
        path.join(projectRoot, "ads", "snapshots"),
        mapping,
        remoteSymbol
      )
    : undefined;
  const generated = readGeneratedProof(
    readIfPresent(path.join(projectRoot, "src", "generated", "ads_generated.st")),
    mapping,
    snapshot ? structuredTextType(snapshot) : undefined
  );
  const runtimeAdsTables = runtimeToml.match(
    /^\s*\[runtime\.ads\]\s*(?:#.*)?$/gm
  ) || [];
  const runtimeAdsBody = runtimeAdsTableBody(runtimeToml);
  return {
    ads_toml_present: adsToml.length > 0,
    selected_remote_symbol_present: selectedPoint.pointExact,
    selected_point_mapping_exact: selectedPoint.pointExact,
    selected_connection_route_exact: selectedPoint.routeExact,
    selected_symbol_snapshot_present: Boolean(snapshot),
    selected_snapshot_structural: Boolean(snapshot),
    generated_st_present: generated.typedLocal && generated.qualityMapping,
    generated_typed_local_declaration: generated.typedLocal,
    generated_quality_mapping: generated.qualityMapping,
    runtime_ads_enabled:
      runtimeAdsTables.length === 1 &&
      matchingLines(runtimeAdsBody, /^\s*enabled\s*=\s*true\s*(?:#.*)?$/gm) === 1 &&
      matchingLines(runtimeAdsBody, /^\s*config_path\s*=\s*["']ads\.toml["']\s*(?:#.*)?$/gm) === 1 &&
      matchingLines(runtimeAdsBody, /^\s*enabled\s*=/gm) === 1 &&
      matchingLines(runtimeAdsBody, /^\s*config_path\s*=/gm) === 1 &&
      matchingLines(runtimeAdsBody, /^\s*worker_tick_interval_ms\s*=/gm) === 1,
  };
}

function runtimeAdsTableBody(source) {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const header = lines.findIndex((line) =>
    /^\s*\[runtime\.ads\]\s*(?:#.*)?$/.test(line)
  );
  if (header < 0) return "";
  const endOffset = lines
    .slice(header + 1)
    .findIndex((line) => /^\s*\[[^\]]+\]\s*(?:#.*)?$/.test(line));
  const end = endOffset < 0 ? lines.length : header + 1 + endOffset;
  return lines.slice(header + 1, end).join("\n");
}

function matchingLines(source, pattern) {
  return (source.match(pattern) || []).length;
}

function readIfPresent(file) {
  return fs.existsSync(file) ? fs.readFileSync(file, "utf8") : "";
}

module.exports = { readProjectAdsImport };
