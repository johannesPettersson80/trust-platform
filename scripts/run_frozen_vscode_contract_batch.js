#!/usr/bin/env node
"use strict";

const crypto = require("crypto");
const fs = require("fs");
const Module = require("module");
const path = require("path");
const { spawnSync } = require("child_process");

const repoRoot = path.resolve(__dirname, "..");
const vscodeRoot = path.join(repoRoot, "editors", "vscode");
const runnerPath = "scripts/run_frozen_vscode_contract_batch.js";
const defaultSourcePath =
  "editors/vscode/src/test/suite/ux-shell-contract.test.ts";
const reviewedSourcePaths = new Set([
  defaultSourcePath,
  "editors/vscode/src/test/suite/network-canvas.test.ts",
  "editors/vscode/src/test/suite/runtime-controls-contract.test.ts",
]);
if (process.argv.length > 3) {
  process.stderr.write("usage: run_frozen_vscode_contract_batch.js [test-source]\n");
  process.exit(2);
}
const sourcePath = process.argv[2] || defaultSourcePath;
if (
  !reviewedSourcePaths.has(sourcePath) ||
  sourcePath.split("/").includes("..") ||
  path.resolve(repoRoot, sourcePath) !==
    path.join(repoRoot, ...sourcePath.split("/"))
) {
  process.stderr.write("test-source is not an allowlisted focused suite\n");
  process.exit(2);
}
const sourceStat = fs.lstatSync(path.join(repoRoot, sourcePath));
if (!sourceStat.isFile() || sourceStat.isSymbolicLink()) {
  process.stderr.write("test-source must be a regular non-symlink file\n");
  process.exit(2);
}
const compiledPath = sourcePath
  .replace("editors/vscode/src/", "editors/vscode/out/")
  .replace(/\.ts$/, ".js");
const command = `node ${runnerPath}${
  process.argv[2] ? ` ${sourcePath}` : ""
}`;

const compile = spawnSync("npm", ["run", "compile"], {
  cwd: vscodeRoot,
  encoding: "utf8",
});
if (compile.status !== 0) {
  process.stderr.write(compile.stdout || "");
  process.stderr.write(compile.stderr || "");
  process.exit(compile.status || 1);
}

const originalLoad = Module._load;
class EventEmitter {
  constructor() {
    this.event = () => ({ dispose() {} });
  }
  fire() {}
  dispose() {}
}
Module._load = function load(request, parent, isMain) {
  return request === "vscode"
    ? { EventEmitter }
    : originalLoad.call(this, request, parent, isMain);
};

const Mocha = require(path.join(vscodeRoot, "node_modules", "mocha"));
const mocha = new Mocha({ ui: "tdd", reporter: function SilentReporter() {} });
mocha.addFile(path.join(repoRoot, compiledPath));

const result = {
  schema_version: 1,
  execution: {
    command,
    runner_path: runnerPath,
    runner_sha256: digest(path.join(repoRoot, runnerPath)),
    source_path: sourcePath,
    source_sha256: digest(path.join(repoRoot, sourcePath)),
    compiled_test_path: compiledPath,
    compiled_test_sha256: digest(path.join(repoRoot, compiledPath)),
  },
  tests: [],
  failures: [],
  pending: [],
};

const runner = mocha.run((failureCount) => {
  result.stats = {
    suites: runner.stats.suites,
    tests: runner.stats.tests,
    passes: runner.stats.passes,
    pending: runner.stats.pending,
    failures: runner.stats.failures,
    duration: runner.stats.duration,
  };
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  process.exitCode = failureCount ? 1 : 0;
});

runner.on("test end", (test) => {
  result.tests.push({
    title: test.title,
    duration: Number.isInteger(test.duration) ? test.duration : 0,
    state: test.state || "unknown",
  });
});
runner.on("pending", (test) => {
  result.pending.push({ title: test.title });
});
runner.on("fail", (test, error) => {
  result.failures.push({
    title: test.title,
    message: error && error.message ? error.message : String(error),
  });
});

function digest(file) {
  return `sha256:${crypto
    .createHash("sha256")
    .update(fs.readFileSync(file))
    .digest("hex")}`;
}
