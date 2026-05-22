#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const args = process.argv.slice(2);
const allowedFile = "crates/trust-runtime/src/world.rs";
const allowedMarker = "WORLD_DYNAMIC_TRANSFORM_HANDOFF_ALLOWED";
const forbiddenPattern = /\.set_transform\s*\(/;

function usage() {
  console.error("usage: node scripts/check_world_smoke_transform_handoff.mjs --repo | --fixture <path>");
  process.exit(2);
}

function read(relPath) {
  return fs.readFileSync(path.join(root, relPath), "utf8");
}

function lineNumbersWith(source, pattern) {
  return source
    .split(/\r?\n/)
    .map((line, index) => ({ line, number: index + 1 }))
    .filter(({ line }) => pattern.test(line))
    .map(({ number }) => number);
}

if (args[0] === "--repo" && args.length === 1) {
  const source = read(allowedFile);
  const markerCount = (source.match(new RegExp(allowedMarker, "g")) || []).length;
  const setTransformLines = lineNumbersWith(source, forbiddenPattern);
  if (markerCount !== 1) {
    console.error(`expected exactly one ${allowedMarker} marker in ${allowedFile}, found ${markerCount}`);
    process.exit(1);
  }
  if (setTransformLines.length !== 1) {
    console.error(
      `expected exactly one dynamic-body scene.set_transform write in ${allowedFile}, found ${setTransformLines.length}: ${setTransformLines.join(", ")}`
    );
    process.exit(1);
  }
  console.log(`${allowedFile}:${setTransformLines[0]} is the only world-smoke dynamic-body transform handoff`);
  process.exit(0);
}

if (args[0] === "--fixture" && args.length === 2) {
  const fixture = args[1];
  const source = read(fixture);
  const lines = lineNumbersWith(source, forbiddenPattern);
  if (lines.length > 0) {
    console.error(
      `forbidden dynamic-body transform write in ${fixture}:${lines.join(", ")}; use trust_runtime::world::apply_rapier_body_pose_to_scena_node`
    );
    process.exit(1);
  }
  console.log(`${fixture} contains no forbidden dynamic-body transform write`);
  process.exit(0);
}

usage();
