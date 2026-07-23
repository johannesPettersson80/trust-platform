"use strict";

const fs = require("fs");
const path = require("path");
const v8 = require("v8");

const repoRoot = requiredDirectory("TRUST_CATALOG_REPO_ROOT");
const selectionPath = requiredFile("TRUST_CATALOG_SELECTION");
const manifestPath = requiredOutput("TRUST_CATALOG_ATTEMPTS_OUT");
const coverageRoot = requiredDirectory("NODE_V8_COVERAGE");
const Mocha = require(path.join(repoRoot, "editors", "vscode", "node_modules", "mocha"));

function run() {
  const selection = loadSelection(selectionPath);
  const mocha = new Mocha({ ui: "tdd", color: true });
  registerSelectedTests(mocha, selection);
  const attempts = [];
  const active = new Map();
  return new Promise((resolve, reject) => {
    const runner = mocha.run((failures) => {
      const document = {
        schema_version: 1,
        requested_discovery_ids: selection.tests
          .map((test) => test.discovery_id)
          .sort(),
        attempts: attempts.sort((left, right) =>
          left.discovery_id.localeCompare(right.discovery_id)
        ),
      };
      fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
      fs.writeFileSync(manifestPath, `${JSON.stringify(document, null, 2)}\n`, "utf8");
      if (failures > 0) {
        reject(new Error(`${failures} selected VS Code test(s) failed`));
      } else if (attempts.length !== selection.tests.length) {
        reject(
          new Error(
            `selected VS Code attempt count drifted: expected ${selection.tests.length}, got ${attempts.length}`
          )
        );
      } else {
        resolve();
      }
    });
    runner.on("test", (test) => {
      v8.takeCoverage();
      active.set(test, {
        started: process.hrtime.bigint(),
        baseline: coverageFiles(coverageRoot),
      });
    });
    runner.on("test end", (test) => {
      const state = active.get(test);
      if (!state) {
        throw new Error(`coverage state missing for ${test.fullTitle()}`);
      }
      v8.takeCoverage();
      const current = coverageFiles(coverageRoot);
      const rawCoverageFiles = [...current]
        .filter((file) => !state.baseline.has(file))
        .sort();
      const discoveryId = test.__trustCatalogDiscoveryId;
      if (typeof discoveryId !== "string" || discoveryId.length === 0) {
        throw new Error(`catalog identity missing for ${test.fullTitle()}`);
      }
      attempts.push({
        discovery_id: discoveryId,
        attempt_id: `vscode:${discoveryId}`,
        exit_code: test.state === "passed" ? 0 : 1,
        duration_ms: Math.max(
          0,
          Number((process.hrtime.bigint() - state.started) / 1000000n)
        ),
        raw_coverage_files: rawCoverageFiles,
      });
      active.delete(test);
    });
  });
}

function registerSelectedTests(mocha, selection) {
  const selectedByFile = new Map();
  for (const record of selection.tests) {
    const records = selectedByFile.get(record.compiled_file) || [];
    records.push(record);
    selectedByFile.set(record.compiled_file, records);
  }
  for (const [compiledFile, records] of [...selectedByFile.entries()].sort()) {
    const absolute = path.resolve(repoRoot, compiledFile);
    if (!isWithin(absolute, path.join(repoRoot, "editors", "vscode", "out", "test"))) {
      throw new Error(`compiled VS Code test escapes output test root: ${compiledFile}`);
    }
    const before = allTests(mocha.suite).length;
    mocha.suite.emit("pre-require", global, absolute, mocha);
    require(absolute);
    const added = allTests(mocha.suite).slice(before);
    for (const record of records) {
      const matches = added.filter((test) => test.title === record.title);
      if (matches.length !== 1) {
        throw new Error(
          `${record.discovery_id} title resolution expected one test, got ${matches.length}`
        );
      }
      if (matches[0].__trustCatalogDiscoveryId) {
        throw new Error(`${record.discovery_id} resolved an already selected test`);
      }
      matches[0].__trustCatalogDiscoveryId = record.discovery_id;
    }
  }
  pruneSuite(mocha.suite);
  const selected = allTests(mocha.suite);
  if (selected.length !== selection.tests.length) {
    throw new Error(
      `selected VS Code suite expected ${selection.tests.length} tests, got ${selected.length}`
    );
  }
}

function pruneSuite(suite) {
  suite.tests = suite.tests.filter(
    (test) => typeof test.__trustCatalogDiscoveryId === "string"
  );
  for (const child of suite.suites) {
    pruneSuite(child);
  }
  suite.suites = suite.suites.filter(
    (child) => child.tests.length > 0 || child.suites.length > 0
  );
}

function allTests(suite) {
  const tests = [];
  suite.eachTest((test) => tests.push(test));
  return tests;
}

function loadSelection(file) {
  const document = JSON.parse(fs.readFileSync(file, "utf8"));
  if (
    !document ||
    document.schema_version !== 1 ||
    !Array.isArray(document.tests) ||
    document.tests.length === 0
  ) {
    throw new Error("VS Code trace selection has invalid shape");
  }
  const seen = new Set();
  for (const record of document.tests) {
    if (
      !record ||
      typeof record.discovery_id !== "string" ||
      typeof record.source_path !== "string" ||
      typeof record.compiled_file !== "string" ||
      typeof record.title !== "string" ||
      record.title.length === 0
    ) {
      throw new Error("VS Code trace selection contains an invalid test");
    }
    if (seen.has(record.discovery_id)) {
      throw new Error(`VS Code trace selection duplicates ${record.discovery_id}`);
    }
    seen.add(record.discovery_id);
  }
  return document;
}

function coverageFiles(directory) {
  return new Set(
    fs
      .readdirSync(directory)
      .filter((name) => name.endsWith(".json"))
      .map((name) => path.join(directory, name))
  );
}

function requiredDirectory(name) {
  const value = process.env[name];
  if (!value || !fs.statSync(value).isDirectory()) {
    throw new Error(`${name} must name an existing directory`);
  }
  return path.resolve(value);
}

function requiredFile(name) {
  const value = process.env[name];
  if (!value || !fs.statSync(value).isFile()) {
    throw new Error(`${name} must name an existing file`);
  }
  return path.resolve(value);
}

function requiredOutput(name) {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} must name an output file`);
  }
  return path.resolve(value);
}

function isWithin(candidate, directory) {
  const relative = path.relative(path.resolve(directory), path.resolve(candidate));
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

module.exports = { run };
