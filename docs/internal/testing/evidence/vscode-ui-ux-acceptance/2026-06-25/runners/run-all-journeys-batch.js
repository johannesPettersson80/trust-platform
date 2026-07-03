#!/usr/bin/env node
// Batch journey acceptance driver.
//
// This intentionally keeps journey evidence in per-journey folders. Many older
// focused runners still write to the legacy 2026-06-25 root, so this wrapper
// snapshots legacy outputs and imports files changed by each helper into the
// current journey folder instead of letting evidence scatter.
const fs = require("fs");
const path = require("path");
const cp = require("child_process");
const pngHygiene = require("./png-hygiene.js");

const repo = process.env.TRUST_REPO || "/home/johannes/projects/trust-platform";
const runnersDir = __dirname;
const legacyRoot = path.resolve(runnersDir, "..");
const today = process.env.TRUST_UX_DATE || new Date().toISOString().slice(0, 10);
const batchRoot =
  process.env.TRUST_UX_BATCH_ROOT ||
  path.join(repo, "docs/internal/testing/evidence/vscode-ui-ux-acceptance", today);
const lspBin = process.env.ST_LSP_TEST_SERVER || path.join(repo, "target/debug/trust-lsp");
const runtimeBin = process.env.ST_RUNTIME_TEST_BIN || path.join(repo, "target/debug/trust-runtime");
const debugBin = process.env.TRUST_DEBUG_BIN || path.join(repo, "target/debug/trust-debug");

const defaultSkip = new Set(
  (process.env.TRUST_UX_SKIP || "J-01,J-02")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
);
const only = new Set(
  (process.env.TRUST_UX_ONLY || "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
);

const JOURNEYS = [
  {
    id: "J-01",
    slug: "install-first-project",
    status: "existing",
    note: "Fresh retest evidence already exists; skip by default unless TRUST_UX_SKIP is empty.",
    commands: [{ runner: "j01-current-first-project-runner.js", envAware: true }],
  },
  {
    id: "J-02",
    slug: "find-open-right-example",
    status: "existing",
    note: "Fresh evidence already exists; skip by default unless TRUST_UX_SKIP is empty.",
    commands: [{ runner: "j02-examples-gallery-runner.js", envAware: true }],
  },
  {
    id: "J-03",
    slug: "write-compile-fix-st",
    commands: [
      { runner: "st-runner.js" },
      { runner: "st-nav-rename-runner.js" },
      { runner: "st-second-pou-runner.js" },
      { runner: "compile-current-runner.js", envAware: true },
    ],
  },
  {
    id: "J-04",
    slug: "run-stop-simulator",
    commands: [
      { runner: "run-runner.js", envAware: true, env: { RUN_WORKFLOW_SCOPE: "J04" } },
      { runner: "lv-simulator-runner.js", envAware: true },
    ],
  },
  {
    id: "J-05",
    slug: "debug-program",
    commands: [{ runner: "dbg-current-runner.js" }],
  },
  {
    id: "J-06",
    slug: "change-code-while-running",
    commands: [
      { runner: "run-runner.js", envAware: true, env: { RUN_WORKFLOW_SCOPE: "J06" } },
      { runner: "run-remote-runner.js" },
    ],
  },
  {
    id: "J-07",
    slug: "live-values-safely",
    commands: [{ runner: "lv-simulator-runner.js", envAware: true }],
  },
  {
    id: "J-08",
    slug: "managed-local-runtime",
    commands: [
      { runner: "dc-setup-runtime-runner.js", envAware: true },
      { runner: "cdp_dc_managed.js", envAware: true },
      { runner: "cdp_managed_run_target.js", envAware: true },
      { runner: "managed-live-values-runner.js", envAware: true },
    ],
  },
  {
    id: "J-09",
    slug: "remote-runtime",
    commands: [
      { runner: "remote-setup-runner.js", envAware: true },
      { runner: "run-remote-runner.js", envAware: true },
      { runner: "j10-remote-runtime-runner.js", envAware: true },
      { runner: "err-runtime-unreachable-runner.js", envAware: true },
    ],
  },
  {
    id: "J-11",
    slug: "add-manage-remove-connection",
    commands: [
      { runner: "dc-core-runner.js", envAware: true },
      { runner: "dc-add-picker-runner.js", envAware: true },
      { runner: "dc-inspector-runner.js", envAware: true },
      { runner: "dc-filter-safety-runner.js", envAware: true },
      { runner: "j11b-add-validate-save-reopen-runner.js", envAware: true },
      { runner: "j11c-disable-remove-endpoint-runner.js", envAware: true },
    ],
  },
  {
    id: "J-12",
    slug: "simulated-loopback-io",
    commands: [
      { runner: "simulated-loopback-add-runner.js", envAware: true },
      { runner: "simulated-loopback-values-runner.js", envAware: true },
      { runner: "lv-simulator-runner.js", envAware: true },
    ],
  },
  {
    id: "J-13",
    slug: "modbus-program",
    commands: [
      { runner: "discover-workflows-runner.js", envAware: true },
      { runner: "modbus-discover-runner.js", envAware: true },
      { runner: "protocol-form-runner.js", envAware: true },
      { runner: "j11b-add-validate-save-reopen-runner.js", envAware: true },
      { runner: "modbus-use-in-st-runner.js", envAware: true },
    ],
    expectedGap: "MB-06 real Modbus hardware remains hardware-gated; the software-server path covers the non-hardware Modbus journey proof.",
  },
  {
    id: "J-14",
    slug: "mqtt-program",
    commands: [
      { runner: "protocol-form-runner.js", envAware: true },
      { runner: "mqtt-validation-runner.js", envAware: true },
      { runner: "mqtt-broker-probe-runner.js", label: "mqtt-broker-probe", envAware: true },
      { runner: "mqtt-broker-probe-runner.js", label: "mqtt-save", envAware: true, env: { MQTT_WORKFLOW: "save" } },
      { runner: "mqtt-use-in-st-runner.js", envAware: true },
    ],
  },
  {
    id: "J-15",
    slug: "opcua-client-program",
    commands: [
      { runner: "opcua-client-browse-save-runner.js", envAware: true },
      { runner: "opcua-client-cert-trust-runner.js", envAware: true },
      { runner: "opcua-client-auth-required-runner.js", envAware: true },
      { runner: "opcua-client-unreachable-runner.js", envAware: true },
      { runner: "opcua-client-use-in-st-runner.js", envAware: true },
    ],
  },
  {
    id: "J-16",
    slug: "opcua-server-expose",
    commands: [
      { runner: "opcua-server-expose-security-runner.js", envAware: true },
      { runner: "opcua-server-live-read-runner.js", envAware: true },
    ],
  },
  {
    id: "J-17",
    slug: "ads-client-program",
    commands: [{ runner: "ads-client-program-runner.js", envAware: true }],
    expectedGap: "ADS client live browse requires TwinCAT/lab proof if no local ADS fixture is available.",
  },
  {
    id: "J-18",
    slug: "ads-server-expose",
    commands: [{ runner: "ads-server-expose-runner.js", envAware: true }],
  },
  {
    id: "J-19",
    slug: "ethercat-channels",
    commands: [
      { runner: "ethercat-channels-runner.js", envAware: true },
      { runner: "discover-workflows-runner.js" },
      { runner: "protocol-form-retest-runner.js", envAware: true },
    ],
    expectedGap: "Real EtherCAT bus row remains hardware-gated unless lab hardware is attached.",
  },
  {
    id: "J-20",
    slug: "gpio-lines",
    commands: [
      { runner: "protocol-form-retest-runner.js", envAware: true },
      { runner: "cdp_gpio_local.js", envAware: true },
      { runner: "cdp_gpio_connected.js", envAware: true },
      { runner: "discover-workflows-runner.js" },
    ],
    expectedGap: "Real GPIO read/write rows remain hardware-gated unless a lab Raspberry Pi is attached.",
  },
  {
    id: "J-23",
    slug: "advanced-integrations",
    commands: [
      { runner: "pal-advanced-runner.js", envAware: true },
      { runner: "cdp_protocol_forms2.js", envAware: true },
    ],
    expectedGap: "Advanced integrations must stay clearly advanced and configured-only if no runtime proof exists.",
  },
  {
    id: "J-24",
    slug: "hmi",
    commands: [
      { runner: "hmi-create-runner.js", envAware: true },
      { runner: "hmi-current-runner.js", envAware: true },
      { runner: "hmi-write-browser-runner.js", envAware: true },
      { runner: "cdp_hmi_pages.js", envAware: true },
      { runner: "cdp_hmi_trends.js", envAware: true },
    ],
  },
  {
    id: "J-25",
    slug: "native-test-explorer",
    commands: [{ runner: "st-test-workflows-runner.js", envAware: true }],
  },
  {
    id: "J-26",
    slug: "visual-logic",
    commands: [
      { runner: "vis-workflow-runner.js", envAware: true },
      { runner: "vis-theme-runner.js", label: "vis-theme-dark", envAware: true, env: { VIS_THEME: "dark", VIS_WIDTH: "960", VIS_HEIGHT: "760" } },
      { runner: "vis-theme-runner.js", label: "vis-theme-light", envAware: true, env: { VIS_THEME: "light", VIS_WIDTH: "960", VIS_HEIGHT: "760" } },
      { runner: "vis-theme-runner.js", label: "vis-theme-high-contrast", envAware: true, env: { VIS_THEME: "hc", VIS_WIDTH: "960", VIS_HEIGHT: "760" } },
      { runner: "vis-runner.js", envAware: true },
      { runner: "vis-sidebar-runner.js", envAware: true },
    ],
  },
  {
    id: "J-27C",
    slug: "palette-orphan-audit",
    commands: [
      { runner: "palette-shot-runner.js" },
      { runner: "pal-title-audit-runner.js" },
      { runner: "pal-trust-twin-exclusion-runner.js" },
      { runner: "pal-advanced-runner.js" },
    ],
  },
  {
    id: "J-28",
    slug: "product-wide-polish",
    commands: [
      { runner: "vis-theme-runner.js" },
      { runner: "cdp_canvas_inspector_light.js" },
      { runner: "cdp_r2_live_values_light.js" },
      { runner: "dc-home-sidebar-min-width-runner.js" },
      { runner: "dc-loading-state-runner.js" },
    ],
  },
  {
    id: "J-29",
    slug: "existing-nontrust-project",
    commands: [{ runner: "fp06-nontrust-runner.js" }],
  },
  {
    id: "J-31",
    slug: "settings-runtime-settings",
    commands: [{ runner: "settings-targeted-runner.js" }],
  },
  {
    id: "J-32",
    slug: "libraries",
    commands: [
      { runner: "libraries-runner.js" },
      { runner: "library-use-runner.js" },
      { runner: "library-motion-runner.js" },
    ],
  },
  {
    id: "J-Deploy",
    slug: "deploy-device",
    commands: [{ runner: "j01-current-first-project-runner.js", envAware: true }],
    expectedGap: "Deploy is intentionally visible but disabled until backend deploy support is real.",
  },
];

function safeName(name) {
  return name.replace(/[^A-Za-z0-9_.-]+/g, "-");
}

function sh(command, args, options = {}) {
  return cp.spawnSync(command, args, {
    cwd: repo,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
}

function expectedArchPattern() {
  const arch = sh("uname", ["-m"]).stdout.trim();
  if (arch === "aarch64" || arch === "arm64") return { arch, pattern: /aarch64|arm64|ARM aarch64/i };
  if (arch === "x86_64" || arch === "amd64") return { arch, pattern: /x86-64|x86_64/i };
  throw new Error(`Unsupported runner host architecture: ${arch}`);
}

function preflightBinary(name, file) {
  if (!fs.existsSync(file)) throw new Error(`Missing ${name}: ${file}`);
  fs.accessSync(file, fs.constants.X_OK);
  const fileText = sh("file", [file]).stdout.trim();
  const expected = expectedArchPattern();
  if (!expected.pattern.test(fileText)) {
    throw new Error(`${name} architecture mismatch: uname=${expected.arch}; file=${fileText}`);
  }
  const version = sh(file, ["--version"], { timeout: 10000 });
  if (version.status !== 0) {
    throw new Error(`${name} --version failed: ${version.stderr || version.stdout}`);
  }
  return { name, file, fileText, version: version.stdout.trim() || version.stderr.trim() };
}

function listFiles(dir) {
  if (!fs.existsSync(dir)) return new Map();
  const out = new Map();
  const stack = [dir];
  while (stack.length) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
      } else if (entry.isFile()) {
        const stat = fs.statSync(full);
        out.set(path.relative(dir, full), { mtimeMs: stat.mtimeMs, size: stat.size });
      }
    }
  }
  return out;
}

function changedFiles(base, before) {
  const after = listFiles(base);
  const changed = [];
  for (const [rel, stat] of after) {
    const old = before.get(rel);
    if (!old || old.mtimeMs !== stat.mtimeMs || old.size !== stat.size) {
      changed.push(rel);
    }
  }
  return changed.sort();
}

function copyChanged(base, rels, destBase) {
  const copied = [];
  for (const rel of rels) {
    const src = path.join(base, rel);
    const dest = path.join(destBase, rel);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.copyFileSync(src, dest);
    copied.push(path.relative(destBase, dest));
  }
  return copied;
}

function writeReport(journeyRoot, result) {
  const lines = [];
  lines.push(`# ${result.id} - ${result.slug}`);
  lines.push("");
  lines.push(`Status: ${result.status}`);
  lines.push("");
  if (result.expectedGap) {
    lines.push(`Expected gap: ${result.expectedGap}`);
    lines.push("");
  }
  lines.push("## Commands");
  lines.push("");
  for (const command of result.commands) {
    const runnerLabel = command.label ? `${command.label} (${command.runner})` : command.runner;
    lines.push(`- ${runnerLabel}: ${command.status === 0 ? "passed" : `failed (${command.status})`}`);
    if (command.note) lines.push(`  - ${command.note}`);
  }
  lines.push("");
  lines.push("## Evidence");
  lines.push("");
  const screenshots = listFiles(path.join(journeyRoot, "screenshots-raw"));
  for (const rel of [...screenshots.keys()].sort()) {
    lines.push(`- screenshots-raw/${rel}`);
  }
  const json = listFiles(path.join(journeyRoot, "json"));
  for (const rel of [...json.keys()].sort()) {
    lines.push(`- json/${rel}`);
  }
  const legacy = listFiles(path.join(journeyRoot, "legacy-captures"));
  if (legacy.size) {
    lines.push("");
    lines.push("Imported legacy-runner files:");
    for (const rel of [...legacy.keys()].sort()) {
      lines.push(`- legacy-captures/${rel}`);
    }
  }
  lines.push("");
  lines.push("## Review State");
  lines.push("");
  if (result.status === "passed") {
    lines.push("Runner batch completed. This is provisional evidence only; the user has not accepted this journey.");
  } else if (result.status === "skipped") {
    lines.push(result.note || "Skipped by batch configuration.");
  } else {
    lines.push("Runner batch did not complete cleanly. Treat this journey as finding_open until the failure is triaged and rerun.");
  }
  fs.writeFileSync(path.join(journeyRoot, "report.md"), lines.join("\n") + "\n");
}

function runJourney(journey) {
  const result = {
    id: journey.id,
    slug: journey.slug,
    expectedGap: journey.expectedGap,
    status: "passed",
    startedAt: new Date().toISOString(),
    commands: [],
  };

  if (only.size && !only.has(journey.id)) {
    result.status = "skipped";
    result.note = "Not included in this TRUST_UX_ONLY batch; existing journey evidence was left untouched.";
    return result;
  }

  const journeyRoot = path.join(batchRoot, `${journey.id}-${journey.slug}`);
  fs.mkdirSync(path.join(journeyRoot, "screenshots-raw"), { recursive: true });
  fs.mkdirSync(path.join(journeyRoot, "json"), { recursive: true });
  fs.mkdirSync(path.join(journeyRoot, "logs"), { recursive: true });
  const pngRegistry = new Map();

  if (!only.size && defaultSkip.has(journey.id)) {
    result.status = "skipped";
    result.note = journey.note || "Skipped by TRUST_UX_SKIP.";
    writeReport(journeyRoot, result);
    fs.writeFileSync(path.join(journeyRoot, "batch-run.json"), JSON.stringify(result, null, 2));
    return result;
  }

  for (const command of journey.commands) {
    const runnerPath = path.join(runnersDir, command.runner);
    const started = Date.now();
    const legacyBefore = {
      screenshots: listFiles(path.join(legacyRoot, "screenshots-raw")),
      json: listFiles(path.join(legacyRoot, "json")),
    };
    const env = {
      ...process.env,
      TRUST_REPO: repo,
      TRUST_UX_EVIDENCE_ROOT: journeyRoot,
      TRUST_UX_SCREENSHOTS_DIR: path.join(journeyRoot, "screenshots-raw"),
      TRUST_UX_JSON_DIR: path.join(journeyRoot, "json"),
      ST_LSP_TEST_SERVER: lspBin,
      ST_RUNTIME_TEST_BIN: runtimeBin,
      TRUST_DEBUG_BIN: debugBin,
      ...(command.env || {}),
    };
    const args = ["-a", "-s", "-screen 0 1920x1080x24", "node", runnerPath];
    const proc = cp.spawnSync("xvfb-run", args, {
      cwd: repo,
      env,
      encoding: "utf8",
      maxBuffer: 40 * 1024 * 1024,
    });
    const logName = `${safeName(command.label || command.runner)}.log`;
    fs.writeFileSync(
      path.join(journeyRoot, "logs", logName),
      [
        `$ xvfb-run ${args.map((arg) => JSON.stringify(arg)).join(" ")}`,
        "",
        "STDOUT:",
        proc.stdout || "",
        "",
        "STDERR:",
        proc.stderr || "",
      ].join("\n")
    );

    const entry = {
      runner: command.runner,
      label: command.label,
      status: proc.status,
      signal: proc.signal,
      durationMs: Date.now() - started,
      log: path.join("logs", logName),
      envAware: !!command.envAware,
      imported: {},
    };
    if (!command.envAware) {
      const importBase = path.join(journeyRoot, "legacy-captures", safeName(command.runner));
      entry.imported.screenshots = copyChanged(
        path.join(legacyRoot, "screenshots-raw"),
        changedFiles(path.join(legacyRoot, "screenshots-raw"), legacyBefore.screenshots),
        path.join(importBase, "screenshots-raw")
      );
      entry.imported.json = copyChanged(
        path.join(legacyRoot, "json"),
        changedFiles(path.join(legacyRoot, "json"), legacyBefore.json),
        path.join(importBase, "json")
      );
    }
    entry.strippedPngFiles =
      pngHygiene.stripTree(path.join(journeyRoot, "screenshots-raw")) +
      pngHygiene.stripTree(path.join(journeyRoot, "legacy-captures"));
    try {
      const validation = pngHygiene.validateCaptureTree(journeyRoot, {
        duplicateRegistry: pngRegistry,
        expectedWidth: command.env && command.env.TRUST_UX_EXPECTED_WIDTH,
        expectedHeight: command.env && command.env.TRUST_UX_EXPECTED_HEIGHT,
      });
      entry.validPngFiles = validation.valid.length;
    } catch (error) {
      entry.pixelValidation = {
        status: "failed",
        errors: error.errors || [error.message || String(error)],
      };
      result.status = "failed";
    }
    result.commands.push(entry);
    fs.writeFileSync(path.join(journeyRoot, "batch-run.json"), JSON.stringify(result, null, 2));
    if (proc.status !== 0) {
      result.status = "failed";
      break;
    }
    if (entry.pixelValidation && entry.pixelValidation.status === "failed") {
      break;
    }
  }
  result.finishedAt = new Date().toISOString();
  writeReport(journeyRoot, result);
  fs.writeFileSync(path.join(journeyRoot, "batch-run.json"), JSON.stringify(result, null, 2));
  return result;
}

function main() {
  fs.mkdirSync(batchRoot, { recursive: true });
  const preflight = [
    preflightBinary("trust-lsp", lspBin),
    preflightBinary("trust-runtime", runtimeBin),
    preflightBinary("trust-debug", debugBin),
  ];
  const results = [];
  for (const journey of JOURNEYS) {
    console.log(`[journey] ${journey.id} ${journey.slug}`);
    const result = runJourney(journey);
    console.log(`[journey] ${journey.id} ${result.status}`);
    results.push(result);
  }
  const summary = {
    batchRoot,
    preflight,
    startedAt: results[0] && results[0].startedAt,
    finishedAt: new Date().toISOString(),
    totals: results.reduce((acc, result) => {
      acc[result.status] = (acc[result.status] || 0) + 1;
      return acc;
    }, {}),
    results: results.map((result) => ({
      id: result.id,
      slug: result.slug,
      status: result.status,
      commands: result.commands.map((command) => ({
        runner: command.runner,
        status: command.status,
        durationMs: command.durationMs,
      })),
      expectedGap: result.expectedGap,
      note: result.note,
    })),
  };
  fs.writeFileSync(path.join(batchRoot, "journey-batch-summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (results.some((result) => result.status === "failed")) {
    process.exitCode = 1;
  }
}

main();
