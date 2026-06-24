import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

import {
  exampleQuickPickItems,
  hardwareBadge,
  parseManifest,
} from "../../examples/model";
import {
  setUpRuntimeOptions,
  V1_SETUP_CAPS,
} from "../../networkCanvas/webview/setUpRuntime";
import { pickAuthToken } from "../../runtimeAuthModel";
import { summarizeCheck } from "../../checkProgramModel";

// v5 "complete PLC IDE shell" contract guards (vscode-ux-overhaul-plan.md §0.5/§0.6/§9). This file holds
// the package.json + source invariants for the shell: palette cleanup, no user-facing Communication
// panel, the two sidebar states, no "Network Canvas" jargon, examples manifest, etc. The Run-card MODEL
// assertions live in runtime-controls-contract.test.ts.

type MenuItem = { command?: string; when?: string; group?: string };
type Pkg = {
  contributes?: {
    commands?: Array<{ command?: string; title?: string }>;
    menus?: {
      commandPalette?: MenuItem[];
      "editor/title"?: MenuItem[];
      "view/title"?: MenuItem[];
      "view/item/context"?: MenuItem[];
    };
    viewsContainers?: { activitybar?: Array<{ id?: string }> };
    views?: Record<string, Array<{ id?: string; type?: string }>>;
    viewsWelcome?: Array<{ view?: string; contents?: string }>;
  };
};

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function workspaceRoot(): string {
  return path.resolve(extensionRoot(), "..", "..");
}

function loadPackageJson(): Pkg {
  return JSON.parse(
    fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8")
  ) as Pkg;
}

function readSrc(file: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", file), "utf8");
}

function paletteHidden(pkg: Pkg, command: string): boolean {
  const entries = pkg.contributes?.menus?.commandPalette ?? [];
  return entries.some((item) => item.command === command && item.when === "false");
}

function commandTitles(pkg: Pkg): Map<string, string> {
  const map = new Map<string, string>();
  for (const command of pkg.contributes?.commands ?? []) {
    if (command.command) {
      map.set(command.command, command.title ?? "");
    }
  }
  return map;
}

// The leaked palette commands the v5 cleanup hides (§0.5.6). Each stays REGISTERED (escape hatch) but is
// hidden from Ctrl+Shift+P so core flows route through visible surfaces, not the palette.
const HIDDEN_FROM_PALETTE = [
  "trust-lsp.communication.openPanel", // "Communication" — legacy panel, superseded by Devices & Connections
  "trust-lsp.debug.openIoPanel", // "Open Runtime Panel" — reached via the Live Values launcher
  "trust-lsp.debug.start", // "Start Debugging" — F5 uses the debugger, not the palette
  "trust-lsp.debug.attach", // "Attach Debugger"
  "trust-lsp.debug.reload", // "Hot Reload" — Run bar Apply changes drives this
  "trust-lsp.hmi.init", // raw HMI init — reached via the adaptive HMI launcher
  "trust-lsp.hmi.refreshFromDescriptor", // raw HMI refresh
];

const ADS_HIDDEN = [
  "trust-lsp.ads.openPanel",
  "trust-lsp.ads.server.openPanel",
  "trust-lsp.ads.addDevice",
  "trust-lsp.ads.diagnose",
  "trust-lsp.ads.importSymbols",
  "trust-lsp.ads.addRoute",
];

suite("Phase 1 — palette cleanup (v5 shell)", () => {
  test("the leaked core commands are hidden from the command palette (when:false)", () => {
    const pkg = loadPackageJson();
    for (const command of HIDDEN_FROM_PALETTE) {
      assert.ok(
        paletteHidden(pkg, command),
        `${command} must be hidden from the command palette (when:false)`
      );
    }
  });

  test("the six ADS commands stay hidden escapes", () => {
    const pkg = loadPackageJson();
    for (const command of ADS_HIDDEN) {
      assert.ok(paletteHidden(pkg, command), `${command} must remain palette-hidden`);
    }
  });

  test("hidden commands are still REGISTERED (escape hatches, not deleted)", () => {
    const titles = commandTitles(loadPackageJson());
    for (const command of [...HIDDEN_FROM_PALETTE, ...ADS_HIDDEN]) {
      assert.ok(
        titles.has(command),
        `${command} must remain a registered command (hidden from palette, not removed)`
      );
    }
  });

  test("the Communication panel is no longer user-facing", () => {
    const pkg = loadPackageJson();
    // Hidden from the palette …
    assert.ok(
      paletteHidden(pkg, "trust-lsp.communication.openPanel"),
      "Communication must be palette-hidden"
    );
    // … and not surfaced by any menu (editor/title, view/title, view item context).
    const menus = pkg.contributes?.menus ?? {};
    const surfaced = [
      ...(menus["editor/title"] ?? []),
      ...(menus["view/title"] ?? []),
      ...(menus["view/item/context"] ?? []),
    ].some((item) => item.command === "trust-lsp.communication.openPanel");
    assert.ok(
      !surfaced,
      "Communication must not be reachable from any visible menu surface"
    );
  });
});

suite("Phases 2–3 — naming + nav (v5 shell)", () => {
  test("the graph is user-facing 'Devices & Connections', never 'Network Canvas'", () => {
    const titles = commandTitles(loadPackageJson());
    assert.strictEqual(
      titles.get("trust-lsp.networkCanvas.open"),
      "Devices & Connections",
      "the canvas command title must read 'Devices & Connections'"
    );
  });

  test("NO user-facing command title contains the jargon 'Network Canvas'", () => {
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/network canvas/i.test(title),
        `${command} title must not contain 'Network Canvas' (got "${title}")`
      );
    }
  });
});

suite("Phase 5b — examples manifest + bundle (v5 shell)", () => {
  const EXAMPLES_DIR = path.join(extensionRoot(), "media", "examples");

  function manifestEntries() {
    const raw = JSON.parse(
      fs.readFileSync(path.join(EXAMPLES_DIR, "manifest.json"), "utf8")
    );
    return parseManifest(raw);
  }

  test("the manifest parses and ships the curated starters", () => {
    const ids = manifestEntries().map((entry) => entry.id);
    for (const id of [
      "empty-simulator",
      "conveyor",
      "twincat-ads",
      "raspberry-pi",
      "hmi-starter",
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
        path.join("src", "Main.st"),
      ]) {
        assert.ok(
          fs.existsSync(path.join(dir, file)),
          `example '${entry.id}' must bundle ${file}`
        );
      }
    }
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

  test("hardware badges map to the user-facing requirement labels", () => {
    assert.strictEqual(hardwareBadge("none"), "No hardware");
    assert.strictEqual(hardwareBadge("twincat"), "Requires TwinCAT");
    assert.strictEqual(hardwareBadge("raspberrypi"), "Requires Raspberry Pi");
  });

  test("the quick-pick items carry the badge as the description", () => {
    const items = exampleQuickPickItems(manifestEntries());
    const conveyor = items.find((item) => item.id === "conveyor");
    assert.ok(conveyor, "conveyor must be offered");
    assert.strictEqual(conveyor?.description, "No hardware");
    const ads = items.find((item) => item.id === "twincat-ads");
    assert.strictEqual(ads?.description, "Requires TwinCAT");
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

suite("Phase 0 — packaged runtime tools (v5 shell)", () => {
  test("release VSIX bundles trust-runtime beside trust-lsp and trust-debug", () => {
    const releaseWorkflow = fs.readFileSync(
      path.join(workspaceRoot(), ".github", "workflows", "release.yml"),
      "utf8"
    );
    for (const binary of ["trust-lsp", "trust-debug", "trust-runtime"]) {
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary} editors/vscode/bin/`),
        `Unix VSIX packaging must copy ${binary} into editors/vscode/bin`
      );
      assert.ok(
        releaseWorkflow.includes(`cp target/\${{ matrix.target }}/release/${binary}.exe editors/vscode/bin/`),
        `Windows VSIX packaging must copy ${binary}.exe into editors/vscode/bin`
      );
    }
  });
});

suite("Phase 4 — Live Values (v5 shell)", () => {
  test("the values surface is named 'Live Values' (not 'Structured Text Runtime')", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      /createWebviewPanel\(\s*"trust-io-panel",\s*"Live Values"/.test(host),
      "the panel title must be 'Live Values'"
    );
    assert.ok(host.includes("<title>Live Values</title>"), "the HTML title must be 'Live Values'");
  });

  test("write / force / release are preserved (NOT read-only)", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(host.includes("trust-lsp.debug.io.write"), "write preserved");
    assert.ok(host.includes("trust-lsp.debug.io.force"), "force preserved");
    assert.ok(host.includes("trust-lsp.debug.io.release"), "release preserved");
  });

  test("Release all forces exists end-to-end (button + message + host loop)", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    assert.ok(host.includes('id="releaseAllForces"'), "toolbar has the Release all forces button");
    assert.ok(host.includes("async function releaseAllForces"), "host releases every force");
    assert.ok(host.includes('case "releaseAllForces"'), "host handles the releaseAllForces message");
    assert.ok(
      web.includes('type: "releaseAllForces"'),
      "the webview posts releaseAllForces with the forced addresses"
    );
  });

  test("Force/Unforce work on remote attach too — the old 'not available' gate is removed", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    // The backend now forwards io.force/io.unforce via attach (bbe4dacf2), so the remote-only block is
    // gone — leaving it would be a FALSE limitation. Force/release flow on sim AND remote; the runtime
    // authorizes by role and the catch surfaces any error.
    assert.ok(
      !host.includes("REMOTE_FORCE_UNAVAILABLE") && !host.includes("isRemoteTarget"),
      "the remote-only force/release block must be removed"
    );
    assert.ok(
      !/not available for remote targets yet/i.test(host) &&
        !/not available for remote targets yet/i.test(web),
      "no stale 'not available for remote targets yet' copy remains"
    );
    assert.ok(
      !/allowForce:\s*!remote/.test(web) && !/allowRelease:\s*!remote/.test(web),
      "the webview must NOT disable force/release for remote targets"
    );
    // Still wired (sim + remote) and still surfaces backend errors honestly.
    assert.ok(
      host.includes("trust-lsp.debug.io.force") &&
        host.includes("trust-lsp.debug.io.release"),
      "force/release commands stay wired"
    );
  });

  test("forced values are always visibly marked", () => {
    const host = readSrc("ioPanel.ts");
    const web = readSrc("ioPanel.webview.js");
    assert.ok(host.includes(".forced-badge"), "CSS marks forced values");
    assert.ok(
      web.includes('"forced-badge"') && web.includes('"FORCED"'),
      "the webview renders a FORCED badge on forced rows"
    );
  });
});

suite("Phase 7 — Devices & Connections (shared run-target + naming)", () => {
  const panel = () => readSrc("networkCanvas/networkCanvasPanel.ts");

  test("the canvas panel is user-facing 'Devices & Connections', never 'Network Canvas'", () => {
    const src = panel();
    assert.ok(
      src.includes('"Devices & Connections"'),
      "the panel title must be 'Devices & Connections'"
    );
    assert.ok(
      !src.includes("Structured Text: Network Canvas") &&
        !src.includes("<title>Network Canvas"),
      "no user-facing 'Network Canvas' title"
    );
  });

  test("no user-facing 'Network Canvas' anywhere it renders or reaches the user (bundle + runtime strings)", () => {
    // The BUILT webview bundle is esbuild output (comments stripped) — any match here is a real rendered
    // string. This is what caught the header/title leaks that source-only guards missed.
    const bundle = fs.readFileSync(
      path.join(extensionRoot(), "media", "networkCanvasWebview.js"),
      "utf8"
    );
    assert.ok(
      !bundle.includes("Network Canvas"),
      "the built Devices & Connections webview must not render 'Network Canvas'"
    );
    // Runtime-facing host strings: graph titles posted to the webview + user-visible messages. Match only
    // quoted/templated literals so internal identifiers (NETWORK_CANVAS_VIEW_TYPE, the command id) are fine.
    for (const file of ["networkCanvas/graphData.ts", "runtimeLifecycle.ts"]) {
      assert.ok(
        !/["'`][^"'`\n]*Network Canvas/.test(readSrc(file)),
        `${file} must not contain a user-facing 'Network Canvas' string`
      );
    }
  });

  test("ONE selected-run-target store, written by the dropdown AND the graph", () => {
    const store = readSrc("selectedRuntime.ts");
    assert.ok(
      store.includes("getSelectedRuntimeId") && store.includes("setSelectedRuntimeId"),
      "a shared selected-runtime store exists"
    );
    assert.ok(
      readSrc("trustHomeView.ts").includes("getSelectedRuntimeId"),
      "the Run bar reads the shared store"
    );
  });

  test("Connect on a runtime node ALSO sets the active Run target", () => {
    const src = panel();
    // In the runtimeConnect handler, after a successful connect, the target is set.
    const connectIdx = src.indexOf('case "runtimeConnect"');
    const disconnectIdx = src.indexOf('case "runtimeDisconnect"');
    const handler = src.slice(connectIdx, disconnectIdx);
    assert.ok(
      handler.includes("setSelectedRuntimeId"),
      "connecting must set the run target"
    );
  });

  test("Set as run target selects WITHOUT connecting", () => {
    const src = panel();
    assert.ok(src.includes('case "setAsRunTarget"'), "the panel handles setAsRunTarget");
    const ctrl = readSrc("networkCanvas/webview/runtimeNodeControls.ts");
    assert.ok(
      ctrl.includes('action: "setAsRunTarget"'),
      "a runtime node offers Set as run target"
    );
  });

  test("'Set up runtime…' wizard is capability-gated (Install/Docker gated in v1)", () => {
    const options = setUpRuntimeOptions(V1_SETUP_CAPS);
    const byId = (id: string) => options.find((option) => option.id === id);
    assert.ok(byId("connect")?.available, "Connect existing is available in v1");
    assert.ok(byId("local")?.available, "Run a runtime on this computer is available in v1");
    assert.ok(
      !byId("install")?.available && !!byId("install")?.reason,
      "Install truST runtime is gated with a reason (phase 11)"
    );
    assert.ok(
      !byId("docker")?.available && !!byId("docker")?.reason,
      "Run in Docker is gated with a reason (phase 12)"
    );
  });
});

suite("Phases 8–10 — honest backend gating (no fakes, no dead buttons)", () => {
  test("no dead 'Send to PLC' / 'Deploy' button exists (phase 13 backend not shipped)", () => {
    // Not a registered palette command …
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/send to plc|deploy to/i.test(title),
        `${command} must not expose a deploy action before the backend exists`
      );
    }
    // … and not a rendered button in any user surface.
    for (const file of ["trustHomeView.ts", "ioPanel.ts"]) {
      assert.ok(
        !/send to plc/i.test(readSrc(file)),
        `${file} must not render a Send to PLC button yet`
      );
    }
  });

  test("the validity line is diagnostics-derived, never a fake 'build OK'", () => {
    // Ignore comments — only code/UI strings count.
    const code = readSrc("trustHomeView.ts")
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");
    assert.ok(code.includes("No known errors"), "passive diagnostics line present");
    assert.ok(
      !/build ok|build succeeded|build successful/i.test(code),
      "must NOT claim an authoritative build until the Check program backend exists (phase 8)"
    );
  });

  test("managed local runtimes are projected into the Run target from the fleet lifecycle (phase 9 landed)", () => {
    const src = readSrc("trustHomeView.ts");
    // The launcher exists now (bbe4dacf2): the Run bar lists real managed runtimes + drives Start/Stop
    // through the fleet lifecycle — no fake static "Local runtime" entry, no false advertising.
    assert.ok(
      src.includes("listManagedRuntimes"),
      "the Run bar must list managed runtimes from the fleet lifecycle"
    );
    assert.ok(
      src.includes("startManagedRuntime") && src.includes("stopManagedRuntime"),
      "a selected managed runtime Start/Stop must drive the fleet lifecycle (we own it)"
    );
    assert.ok(
      src.includes("attachManagedRuntimeAfterStart"),
      "managed Start must use the shared attach helper so Live Values can write/force without manual token setup"
    );
    const helper = readSrc("managedRuntimeSession.ts");
    assert.ok(
      helper.includes("runtimeLifecycleService.connectRemote(result.controlEndpoint)") &&
        helper.includes("setSelectedRuntimeId(name)"),
      "the shared managed-runtime attach helper must attach to the reached endpoint and set the Run target"
    );
    assert.ok(
      !/LOCAL_RUNTIME_SUPPORTED/.test(src),
      "the stale static local-runtime flag must be gone"
    );
  });

  test("managed runtime tokens are imported into SecretStorage before attach", () => {
    const src = readSrc("localRuntime.ts");
    assert.ok(
      src.includes("parseRuntimeControlAuthToken"),
      "managed runtime token must be read from that runtime project's runtime.toml"
    );
    assert.ok(
      src.includes("setControlAuthToken"),
      "managed runtime token must be saved to SecretStorage, not plaintext settings"
    );
    assert.ok(
      !/runtime\.controlAuthToken/.test(src),
      "managed runtime token import must not write the legacy plaintext setting"
    );
  });
});

suite("Phase 6 — Apply changes (simulator-only)", () => {
  test("Apply changes is sim-only, gated on a real source change, wired to hot reload", () => {
    const src = readSrc("trustHomeView.ts");
    // sim-only + running + an actual change
    assert.ok(
      /selected\.kind === "simulator"/.test(src) &&
        /selected\.status === "running"/.test(src) &&
        /this\.sourceChanged/.test(src),
      "canApply must require simulator + running + a real source change"
    );
    // wired to the existing hot reload, not a fake
    assert.ok(
      src.includes("trust-lsp.debug.reload"),
      "Apply changes must drive the hot-reload command"
    );
    // change detection is save-based (honest), and reset on Start/Apply
    assert.ok(
      src.includes("onDidSaveTextDocument") && src.includes("markSourceChanged"),
      "source-change must be detected from an actual ST save"
    );
  });
});

suite("R4 — runtime auth tokens in SecretStorage (security)", () => {
  test("pickAuthToken: SecretStorage value wins; empty falls back to the legacy setting", () => {
    assert.strictEqual(pickAuthToken("sek", "legacy"), "sek");
    assert.strictEqual(pickAuthToken("", "legacy"), "legacy");
    assert.strictEqual(pickAuthToken(undefined, "legacy"), "legacy");
    assert.strictEqual(pickAuthToken("  ", " legacy "), "legacy");
    assert.strictEqual(pickAuthToken(undefined, undefined), undefined);
    assert.strictEqual(pickAuthToken("", ""), undefined);
  });

  test("token read paths use the SecretStorage-backed store, not the raw plaintext setting", () => {
    for (const file of ["runtimeTarget.ts", "runtimeLifecycle.ts"]) {
      const src = readSrc(file);
      assert.ok(
        src.includes("getControlAuthToken"),
        `${file} must read tokens via getControlAuthToken`
      );
      assert.ok(
        !/config\.get<[^>]*>\("runtime\.controlAuthToken"/.test(src),
        `${file} must not read the plaintext controlAuthToken setting directly`
      );
    }
  });

  test("the legacy plaintext token setting is marked legacy + points to the secret store", () => {
    const pkg = fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8");
    const idx = pkg.indexOf("trust-lsp.runtime.controlAuthToken");
    assert.ok(idx >= 0, "the setting still exists (as a fallback)");
    const block = pkg.slice(idx, idx + 400);
    assert.ok(/legacy/i.test(block), "setting description must flag it as legacy");
    assert.ok(/secret store/i.test(block), "setting must point users to the secret store");
  });
});

suite("Phase 8 — Check program (authoritative compile)", () => {
  test("summarizeCheck: passed vs failed wording", () => {
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 3 }),
      "Project check passed — 3 sources, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 1 }),
      "Project check passed — 1 source, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: false, status: "failed", errors: 2, warnings: 1, issues: [] }),
      "Project check failed — 2 errors, 1 warning."
    );
  });

  test("Check program is ONE action (Project menu + palette), not a new run/build surface", () => {
    assert.strictEqual(
      commandTitles(loadPackageJson()).get("trust-lsp.checkProgram"),
      "Check program"
    );
    const view = readSrc("trustHomeView.ts");
    assert.ok(
      view.includes("trust-lsp.checkProgram"),
      "the Project menu must invoke Check program"
    );
    // It must NOT add a competing build/run control to the Run card.
    assert.ok(
      !/id="check"/.test(view),
      "no Check button competing with the single Run action"
    );
  });
});
