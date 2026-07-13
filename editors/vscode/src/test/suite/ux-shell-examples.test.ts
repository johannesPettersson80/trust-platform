import {
  assert,
  fs,
  path,
  exampleQuickPickItems,
  hardwareBadge,
  parseManifest,
  extensionRoot,
  workspaceRoot,
  readSrc,
} from "./ux-shell-contract-fixtures";

suite("Phase 5b — examples manifest + bundle (v5 shell)", () => {
  const EXAMPLES_DIR = path.join(extensionRoot(), "media", "examples");

  function manifestEntries() {
    const raw = JSON.parse(
      fs.readFileSync(path.join(EXAMPLES_DIR, "manifest.json"), "utf8")
    );
    return parseManifest(raw);
  }

  function readLaunchConfig(dir: string): {
    configurations?: Array<{
      type?: string;
      request?: string;
      name?: string;
      program?: string;
    }>;
  } {
    return JSON.parse(
      fs.readFileSync(path.join(dir, ".vscode", "launch.json"), "utf8")
    ) as {
      configurations?: Array<{
        type?: string;
        request?: string;
        name?: string;
        program?: string;
      }>;
    };
  }

  function readWorkspaceSettings(dir: string): Record<string, unknown> {
    return JSON.parse(
      fs.readFileSync(path.join(dir, ".vscode", "settings.json"), "utf8")
    ) as Record<string, unknown>;
  }

  test("the manifest parses and ships the curated starters", () => {
    const ids = manifestEntries().map((entry) => entry.id);
    for (const id of [
      "empty-simulator",
      "conveyor",
      "twincat-ads",
      "raspberry-pi",
      "hmi-starter",
      "plcopen-motion-single-axis",
    ]) {
      assert.ok(ids.includes(id), `manifest must include the '${id}' starter`);
    }
  });
  test("every example folder is a runnable scaffold (the 4 files), bundled in media/", () => {
    for (const entry of manifestEntries()) {
      const dir = path.join(EXAMPLES_DIR, entry.path);
      for (const file of [
        "trust-lsp.toml",
        "runtime.toml",
        "io.toml",
        path.join(".vscode", "launch.json"),
        path.join(".vscode", "settings.json"),
        path.join("src", "Main.st"),
      ]) {
        assert.ok(
          fs.existsSync(path.join(dir, file)),
          `example '${entry.id}' must bundle ${file}`
        );
      }
    }
  });
  test("every bundled example has a native truST Simulator debug configuration", () => {
    for (const entry of manifestEntries()) {
      const dir = path.join(EXAMPLES_DIR, entry.path);
      const launch = readLaunchConfig(dir);
      const config = launch.configurations?.find(
        (candidate) =>
          candidate.type === "structured-text" &&
          candidate.request === "launch" &&
          candidate.name === "truST Simulator"
      );
      assert.ok(
        config,
        `example '${entry.id}' must give VS Code a native truST Simulator launch configuration`
      );
      assert.ok(
        typeof config?.program === "string" &&
          config.program.startsWith("${workspaceFolder}/src/"),
        `example '${entry.id}' launch config must point at its bundled CONFIGURATION source`
      );
      const relativeProgram = config!.program!.replace("${workspaceFolder}/", "");
      assert.ok(
        fs.existsSync(path.join(dir, relativeProgram)),
        `example '${entry.id}' launch program must exist: ${relativeProgram}`
      );
    }
  });
  test("every bundled example hides the native debug status selector", () => {
    for (const entry of manifestEntries()) {
      const dir = path.join(EXAMPLES_DIR, entry.path);
      const settings = readWorkspaceSettings(dir);
      assert.strictEqual(
        settings["debug.showInStatusBar"],
        "never",
        `example '${entry.id}' must keep the truST sidebar as the single run/debug control surface`
      );
    }
  });
  test("new project scaffolding writes the same native debug configuration", () => {
    const source = readSrc("newProject.ts");
    assert.ok(
      source.includes('const LAUNCH_JSON_SOURCE = `') &&
        source.includes('"name": "truST Simulator"') &&
        source.includes('"program": "\\${workspaceFolder}/src/config.st"') &&
        source.includes('vscode.Uri.joinPath(targetUri, ".vscode")') &&
        source.includes('vscode.Uri.joinPath(vscodeUri, "launch.json")') &&
        source.includes('const VSCODE_SETTINGS_SOURCE = `') &&
        source.includes('"debug.showInStatusBar": "never"') &&
        source.includes('vscode.Uri.joinPath(vscodeUri, "settings.json")'),
      "Create project must write launch.json plus settings.json so VS Code has a debugger but does not show a second debug status selector"
    );
  });
  test("journey batch strips raw helper PNG output before validation", () => {
    const runner = fs.readFileSync(
      path.join(
        workspaceRoot(),
        "docs",
        "internal",
        "testing",
        "evidence",
        "vscode-ui-ux-acceptance",
        "2026-06-25",
        "runners",
        "run-all-journeys-batch.js"
      ),
      "utf8"
    );
    assert.ok(
      runner.includes("pngHygiene.stripTree(journeyRoot)") &&
        runner.includes("including diagnostic runner-output copies"),
      "journey batch must strip all PNGs, not only reviewer-facing screenshots-raw"
    );
  });
  test("debug journey fixture has a native truST Simulator launch configuration", () => {
    const dir = path.join(workspaceRoot(), "examples", "network_canvas_demo");
    const launch = readLaunchConfig(dir);
    assert.ok(
      launch.configurations?.some(
        (config) =>
          config.type === "structured-text" &&
          config.request === "launch" &&
          config.name === "truST Simulator" &&
          config.program === "${workspaceFolder}/src/config.st"
      ),
      "network_canvas_demo must not leave the native debug selector at No Configurations"
    );
  });
  test("every example runtime.toml has the sections the runtime parser requires", () => {
    // The parser requires retain/watchdog/fault (Codex review) — a compact file fails to load offline.
    for (const entry of manifestEntries()) {
      const toml = fs.readFileSync(
        path.join(EXAMPLES_DIR, entry.path, "runtime.toml"),
        "utf8"
      );
      for (const section of [
        "[runtime.control]",
        "[runtime.retain]",
        "[runtime.watchdog]",
        "[runtime.fault]",
      ]) {
        assert.ok(
          toml.includes(section),
          `example '${entry.id}' runtime.toml must declare ${section}`
        );
      }
    }
  });
  test("every example instantiates its program in a configuration", () => {
    for (const entry of manifestEntries()) {
      const srcDir = path.join(EXAMPLES_DIR, entry.path, "src");
      const stFiles = fs
        .readdirSync(srcDir)
        .filter((name) => name.toLowerCase().endsWith(".st"));
      const source = stFiles
        .map((name) => fs.readFileSync(path.join(srcDir, name), "utf8"))
        .join("\n");
      assert.ok(
        /\bCONFIGURATION\b/i.test(source),
        `example '${entry.id}' must include a CONFIGURATION so first open is warning-free`
      );
      assert.ok(
        /\bPROGRAM\s+\w+\s+WITH\b/i.test(source),
        `example '${entry.id}' must bind a program instance to a task`
      );
    }
  });
  test("hardware badges map to the user-facing requirement labels", () => {
    assert.strictEqual(hardwareBadge("none"), "No hardware");
    assert.strictEqual(hardwareBadge("twincat"), "Requires TwinCAT");
    assert.strictEqual(hardwareBadge("raspberrypi"), "Requires Raspberry Pi");
  });
  test("example gallery entries carry hardware badges", () => {
    const items = exampleQuickPickItems(manifestEntries());
    const conveyor = items.find((item) => item.id === "conveyor");
    assert.ok(conveyor, "conveyor must be offered");
    assert.strictEqual(conveyor?.description, "No hardware");
    const ads = items.find((item) => item.id === "twincat-ads");
    assert.strictEqual(ads?.description, "Requires TwinCAT");
  });
  test("example gallery has scalable search plus hardware and tag filters", () => {
    const source = readSrc("examples.ts");
    assert.ok(
      source.includes('placeholder="Search examples"') &&
        source.includes("searchWrap.hidden = false"),
      "the example gallery must always expose search; it cannot become unusable when the catalog grows"
    );
    assert.ok(
      source.includes('"no-hardware"') &&
        source.includes('"hardware"') &&
        source.includes("for (const tag of example.tags"),
      "the example gallery must generate hardware and category/tag chips instead of a flat unfiltered list"
    );
    assert.ok(
      source.includes('dataset.filterKind = "hardware"') &&
        source.includes('dataset.filterKind = "category"') &&
        source.includes("hardwareFilter") &&
        source.includes("tagFilter"),
      "hardware and category filters must be separate state so users can combine No hardware with a category"
    );
    assert.ok(
      source.includes("Clear search and filters") &&
        source.includes('hardwareFilter = "all"') &&
        source.includes('tagFilter = "all"') &&
        source.includes('searchInput.value = ""'),
      "the no-match state must provide a visible reset instead of a dead empty surface"
    );
  });
  test("example gallery separates hardware requirements from category labels", () => {
    const source = readSrc("examples.ts");
    assert.ok(
      source.includes(".badge.requires") &&
        source.includes("var(--trust-warn)") &&
        source.includes('hw.className = "badge hardware "'),
      "hardware-required examples must use the shared warning role instead of looking like neutral tags"
    );
    assert.ok(
      source.includes("const TAG_LABELS") &&
        source.includes('ads: "ADS"') &&
        source.includes('raspberrypi: "Raspberry Pi"') &&
        source.includes("titleCaseTag") &&
        source.includes('.split("-")'),
      "category chips must show user-facing labels while keeping stable filter ids"
    );
  });
  test("example copy keeps native prompts and exposes only an acceptance-runner prompt override", () => {
    const source = readSrc("examples.ts");
    assert.ok(source.includes("showOpenDialog"), "normal users must still choose the destination through native VS Code UI");
    assert.ok(source.includes("showInputBox"), "normal users must still name the editable example copy through native VS Code UI");
    assert.ok(
      source.includes("TRUST_UX_EXAMPLE_DESTINATION") &&
        source.includes("TRUST_UX_EXAMPLE_NAME") &&
        source.includes("TRUST_UX_EXAMPLE_OPEN_FOLDER"),
      "the J-01 runner may supply native prompt answers without monkeypatching VS Code APIs"
    );
  });
  test("starter descriptions are compact enough for gallery cards", () => {
    for (const item of exampleQuickPickItems(manifestEntries())) {
      assert.ok(
        item.detail.length <= 80,
        `example '${item.id}' detail is too long for the first-run gallery card: ${item.detail.length} chars`
      );
    }
  });
  test("the PLCopen Motion starter is portable outside the repository", () => {
    const entry = manifestEntries().find((candidate) => candidate.id === "plcopen-motion-single-axis");
    assert.ok(entry, "PLCopen Motion starter must be offered");
    const dir = path.join(EXAMPLES_DIR, entry.path);
    const manifest = fs.readFileSync(path.join(dir, "trust-lsp.toml"), "utf8");
    assert.ok(
      manifest.includes('PLCopenMotionSingleAxis = { path = "libraries/plcopen_motion", version = "0.1.0" }'),
      "PLCopen Motion starter must use a project-relative vendored dependency"
    );
    assert.ok(
      fs.existsSync(path.join(dir, "libraries", "plcopen_motion", "trust-lsp.toml")),
      "PLCopen Motion starter must include the vendored library"
    );
    const source = fs.readFileSync(path.join(dir, "src", "Main.st"), "utf8");
    assert.ok(source.includes("MC_MoveAbsolute"), "starter must show an MC_ motion block");
    assert.ok(source.includes("motion_demo_cycles"), "starter must expose mapped proof values");
    assert.ok(
      !fs.existsSync(path.join(dir, "src", "Globals.st")),
      "starter should stay small enough to read on first open"
    );
  });
  test("no secrets in example fixtures (no tokens/passwords/keys)", () => {
    const offenders: string[] = [];
    const walk = (dir: string) => {
      for (const name of fs.readdirSync(dir)) {
        const full = path.join(dir, name);
        if (fs.statSync(full).isDirectory()) {
          walk(full);
          continue;
        }
        const text = fs.readFileSync(full, "utf8");
        // A real secret has a non-empty value; `auth_token = ""` (empty default in runtime.toml) is fine.
        if (/(auth_token|password|secret|api[_-]?key)\s*[:=]\s*["']?[^"'\s]/i.test(text)) {
          offenders.push(full);
        }
      }
    };
    walk(EXAMPLES_DIR);
    assert.deepStrictEqual(offenders, [], "example fixtures must not contain secrets");
  });
});
