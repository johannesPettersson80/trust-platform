#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const args = process.argv.slice(2);
const allowedFile = "crates/trust-runtime/src/world.rs";
const allowedArmFile = "crates/trust-runtime/src/world/arm.rs";
const jointAllowedFiles = new Set([allowedFile, allowedArmFile]);
const allowedMarker = "WORLD_DYNAMIC_TRANSFORM_HANDOFF_ALLOWED";
const setTransformPattern = /\.set_transform\s*\(/;
const teleportPattern = /\.(set_position|set_translation|set_next_kinematic_position|set_next_kinematic_translation|set_next_kinematic_rotation)\s*\(/;
const kinematicPositionPattern = /KinematicPositionBased|kinematic_position_based\s*\(/;
const sceneReparentPattern = /\.(set_parent|reparent)\s*\(/;
const poseCopyPattern = /copy_carrier_pose_to_workpiece|workpiece\.set_position\s*\([^;]*carrier\.position\s*\(/;
const fixedJointPattern = /FixedJointBuilder::new|\.impulse_joints\s*\.\s*(insert|remove)\s*\(/;
const fkPattern = /compute_fk_for_chain|fk_predicted_position|fk_verifier/i;

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

function sourceFiles(rootDir) {
  const absoluteRoot = path.join(root, rootDir);
  if (!fs.existsSync(absoluteRoot)) {
    return [];
  }
  const files = [];
  for (const entry of fs.readdirSync(absoluteRoot, { withFileTypes: true })) {
    if (["target", "node_modules", ".git"].includes(entry.name)) {
      continue;
    }
    const absolute = path.join(absoluteRoot, entry.name);
    const relative = path.relative(root, absolute).split(path.sep).join("/");
    if (entry.isDirectory()) {
      files.push(...sourceFiles(relative));
    } else if (/\.(rs|mjs|js)$/.test(entry.name)) {
      files.push(relative);
    }
  }
  return files;
}

function forbiddenJointSitesOutsideWorld() {
  return sourceFiles("crates")
    .filter((file) => !jointAllowedFiles.has(file))
    .flatMap((file) => {
      const lines = lineNumbersWith(read(file), fixedJointPattern);
      return lines.map((line) => `${file}:${line}`);
    });
}

function functionBlocks(source) {
  const lines = source.split(/\r?\n/);
  const blocks = [];
  let current = null;
  let depth = 0;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!current && /^\s*(pub\s+)?(async\s+)?fn\s+\w+/.test(line)) {
      current = { start: index + 1, lines: [] };
      depth = 0;
    }
    if (current) {
      current.lines.push(line);
      depth += (line.match(/{/g) || []).length;
      depth -= (line.match(/}/g) || []).length;
      if (depth <= 0 && current.lines.some((blockLine) => blockLine.includes("{"))) {
        blocks.push(current);
        current = null;
      }
    }
  }
  return blocks;
}

function forbiddenFkWriterSites() {
  return [allowedFile, allowedArmFile].flatMap((file) => {
    if (!fs.existsSync(path.join(root, file))) {
      return [];
    }
    return functionBlocks(read(file))
      .filter((block) => {
        const text = block.lines.join("\n");
        return fkPattern.test(text) && (setTransformPattern.test(text) || teleportPattern.test(text));
      })
      .map((block) => `${file}:${block.start}`);
  });
}

if (args[0] === "--repo" && args.length === 1) {
  const source = read(allowedFile);
  const armSource = fs.existsSync(path.join(root, allowedArmFile)) ? read(allowedArmFile) : "";
  const markerCount = (source.match(new RegExp(allowedMarker, "g")) || []).length;
  const setTransformLines = lineNumbersWith(source, setTransformPattern);
  const teleportLines = [
    ...lineNumbersWith(source, teleportPattern).map((line) => `${allowedFile}:${line}`),
    ...lineNumbersWith(armSource, teleportPattern).map((line) => `${allowedArmFile}:${line}`),
  ];
  const kinematicLines = [
    ...lineNumbersWith(source, kinematicPositionPattern).map((line) => `${allowedFile}:${line}`),
    ...lineNumbersWith(armSource, kinematicPositionPattern).map((line) => `${allowedArmFile}:${line}`),
  ];
  const reparentLines = [
    ...lineNumbersWith(source, sceneReparentPattern).map((line) => `${allowedFile}:${line}`),
    ...lineNumbersWith(armSource, sceneReparentPattern).map((line) => `${allowedArmFile}:${line}`),
  ];
  const poseCopyLines = [
    ...lineNumbersWith(source, poseCopyPattern).map((line) => `${allowedFile}:${line}`),
    ...lineNumbersWith(armSource, poseCopyPattern).map((line) => `${allowedArmFile}:${line}`),
  ];
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
  if (teleportLines.length > 0 || kinematicLines.length > 0) {
    console.error(
      `forbidden workpiece rigid-body teleport in world smoke code: ${[...teleportLines, ...kinematicLines].join(", ")}`
    );
    process.exit(1);
  }
  if (reparentLines.length > 0) {
    console.error(`forbidden dynamic-body scene reparent in world smoke code: ${reparentLines.join(", ")}`);
    process.exit(1);
  }
  if (poseCopyLines.length > 0) {
    console.error(`forbidden carrier-to-workpiece pose copy in world smoke code: ${poseCopyLines.join(", ")}`);
    process.exit(1);
  }
  const fkWriterSites = forbiddenFkWriterSites();
  if (fkWriterSites.length > 0) {
    console.error(`forbidden FK-to-transform write in world smoke code: ${fkWriterSites.join(", ")}`);
    process.exit(1);
  }
  const outsideJointSites = forbiddenJointSitesOutsideWorld();
  if (outsideJointSites.length > 0) {
    console.error(
      `forbidden world-smoke workpiece joint construction outside audited world files: ${outsideJointSites.join(", ")}`
    );
    process.exit(1);
  }
  console.log(`${allowedFile}:${setTransformLines[0]} is the only world-smoke dynamic-body transform handoff`);
  process.exit(0);
}

if (args[0] === "--fixture" && args.length === 2) {
  const fixture = args[1];
  const source = read(fixture);
  const poseCopyLines = lineNumbersWith(source, poseCopyPattern);
  if (poseCopyLines.length > 0) {
    console.error(
      `forbidden carrier-to-workpiece pose copy in ${fixture}:${poseCopyLines.join(", ")}; carry must use a Rapier joint`
    );
    process.exit(1);
  }
  const setTransformLines = lineNumbersWith(source, setTransformPattern);
  if (setTransformLines.length > 0 && fkPattern.test(source)) {
    console.error(
      `forbidden FK-to-transform write in ${fixture}:${setTransformLines.join(", ")}; FK is a verifier and must not write visible transforms`
    );
    process.exit(1);
  }
  if (setTransformLines.length > 0) {
    console.error(
      `forbidden dynamic-body transform write in ${fixture}:${setTransformLines.join(", ")}; use trust_runtime::world::apply_rapier_body_pose_to_scena_node`
    );
    process.exit(1);
  }
  const teleportLines = lineNumbersWith(source, teleportPattern);
  const kinematicLines = lineNumbersWith(source, kinematicPositionPattern);
  if (teleportLines.length > 0 || kinematicLines.length > 0) {
    console.error(
      `forbidden workpiece rigid-body teleport in ${fixture}:${[...teleportLines, ...kinematicLines].join(", ")}; workpiece motion must come from Rapier integration`
    );
    process.exit(1);
  }
  const reparentLines = lineNumbersWith(source, sceneReparentPattern);
  if (reparentLines.length > 0) {
    console.error(`forbidden dynamic-body scene reparent in ${fixture}:${reparentLines.join(", ")}`);
    process.exit(1);
  }
  console.log(`${fixture} contains no forbidden dynamic-body transform write`);
  process.exit(0);
}

usage();
