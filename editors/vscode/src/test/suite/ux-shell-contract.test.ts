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
  "trust-lsp.trustTwin.openPanel", // trust-twin 3D panel — internal/experimental, scoped out of the first-user flow
  "trust-lsp.trustTwin.refreshPanel", // trust-twin 3D panel refresh
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

  test("no palette-visible command embeds a 'Structured Text:' category prefix (one truST category)", () => {
    const pkg = loadPackageJson();
    for (const cmd of pkg.contributes?.commands ?? []) {
      if (!cmd.command || paletteHidden(pkg, cmd.command)) {
        continue;
      }
      assert.ok(
        !(cmd.title ?? "").startsWith("Structured Text:"),
        `${cmd.command} (palette-visible) must not embed a category prefix in its title: "${cmd.title}"`
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

  test("stopped/no-session state is beginner-facing and clears stale values", () => {
    const host = readSrc("ioPanel.ts");
    assert.ok(
      host.includes("Start the runtime to see live values."),
      "Live Values must explain the stopped state in user-facing language"
    );
    assert.ok(
      host.includes("function postEmptyIoState"),
      "Live Values must have a single helper for clearing stale I/O rows"
    );
    const requestIoStateBody = host.slice(
      host.indexOf("async function requestIoState"),
      host.indexOf("async function writeInput")
    );
    assert.ok(
      requestIoStateBody.includes("postEmptyIoState();"),
      "a no-session request must clear stale rows before showing stopped guidance"
    );
    const terminateBody = host.slice(
      host.indexOf("vscode.debug.onDidTerminateDebugSession"),
      host.indexOf("vscode.debug.onDidChangeActiveDebugSession")
    );
    assert.ok(
      terminateBody.includes("postEmptyIoState();"),
      "debug session termination must clear stale rows"
    );
    assert.ok(
      !/payload:\s*"No active Structured Text debug session\."/.test(host),
      "Live Values must not display the raw debug-adapter no-session message"
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

  test("refresh does not post through a disposed canvas panel", () => {
    const src = panel();
    assert.ok(
      src.includes("const panelRef = panel;"),
      "refresh must snapshot the current webview panel before any await"
    );
    assert.ok(
      src.includes("if (panel !== panelRef)") && src.includes("return;"),
      "refresh must stop if the panel was disposed/replaced while async work was in flight"
    );
    assert.ok(
      src.includes("panelRef.webview.postMessage"),
      "refresh must post through the stable panel reference, not the mutable global"
    );
  });

  test("node inspector maps raw health ids to user-facing labels", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    assert.ok(
      src.includes("function healthLabel"),
      "NodeInspector must map backend health ids before rendering inspector status rows"
    );
    assert.ok(
      /case "configured_policy":[\s\S]*return "Configured";/.test(src),
      "configured_policy must render as Configured, never as the raw backend enum"
    );
    assert.ok(
      src.includes("healthLabel(health)") &&
        !src.includes('`${health} · ${str(d.detail)}`'),
      "endpoint status rows must use healthLabel(health), not raw health ids"
    );
  });

  test("starting a new canvas drawer clears stale apply errors", () => {
    const host = panel();
    const app = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      host.includes('case "clearApplyResult"') &&
        host.includes("lastApplyResult = undefined"),
      "the canvas host must clear lastApplyResult on request so old faults disappear"
    );
    assert.ok(
      app.includes("function Canvas()") &&
        app.includes("const clearApplyResult = useCallback"),
      "the webview must centralize clearing transient apply state"
    );
    assert.ok(
      app.includes('vscode.postMessage({ type: "clearApplyResult" })'),
      "the webview must tell the host to clear stale apply state, not only local React state"
    );
    assert.ok(
      /onPickSlot:[\s\S]*clearApplyResult\(\);[\s\S]*if \(slot\.add === "device"\)/.test(app),
      "opening a new Add flow must clear stale validation/fault banners"
    );
    assert.ok(
      /onChoose=\{\(protocol\) => \{[\s\S]*clearApplyResult\(\);[\s\S]*setDraft/.test(app),
      "choosing a new protocol form must clear stale validation/fault banners"
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

suite("VIS — visual editors follow the shared Run + Live Values model", () => {
  const visualEditorFiles = [
    "sfc/webview/SfcEditor.tsx",
    "statechart/webview/StateChartEditor.tsx",
    "ladder/webview/LadderEditor.tsx",
    "blockly/webview/BlocklyEditor.tsx",
  ];

  test("visual editors do not render the legacy embedded runtime/I/O panel", () => {
    for (const file of visualEditorFiles) {
      const src = readSrc(file);
      assert.ok(
        !src.includes("StRuntimePanel"),
        `${file} must not import/render StRuntimePanel; use the shared Run card + Live Values surfaces`
      );
      assert.ok(
        !/rightPaneView\s*===\s*"io"|setRightPaneView\("io"\)|>\s*I\/O\s*</.test(src),
        `${file} must not expose a local I/O tab`
      );
      assert.ok(
        !/rightPaneView\s*===\s*"settings"|setRightPaneView\("settings"\)|>\s*Settings\s*</.test(src),
        `${file} must not expose a local runtime settings tab`
      );
      assert.ok(
        !/Open Runtime Panel|Compile Diagnostics/.test(src),
        `${file} must not route users to the old Runtime Panel mental model`
      );
      assert.ok(
        !/MiniMap|<Panel\b/.test(src),
        `${file} must not render default minimap/stat overlays that obscure the program`
      );
    }
  });

  test("product webviews share the same truST theme source", () => {
    assert.ok(
      fs.existsSync(path.join(extensionRoot(), "src", "webview", "theme.ts")),
      "shared React style tokens must live in src/webview/theme.ts"
    );
    assert.ok(
      fs.existsSync(path.join(extensionRoot(), "src", "webview", "theme.css")),
      "shared CSS tokens must live in src/webview/theme.css"
    );
    assert.ok(
      !fs.existsSync(path.join(extensionRoot(), "src", "networkCanvas", "webview", "theme.css")),
      "Devices & Connections must not keep a parallel CSS theme file"
    );

    const expectedImport = 'import "../../webview/theme.css";';
    for (const file of [
      "networkCanvas/webview/NetworkCanvasApp.tsx",
      "sfc/webview/main.tsx",
      "statechart/webview/main.tsx",
      "ladder/webview/main.tsx",
      "blockly/webview/main.tsx",
    ]) {
      const src = readSrc(file);
      assert.ok(
        src.includes(expectedImport),
        `${file} must import the shared CSS theme, not a local theme copy`
      );
      assert.ok(
        !src.includes("trustTheme.css"),
        `${file} must not import the retired private visual-editor theme`
      );
    }

    assert.strictEqual(
      readSrc("networkCanvas/webview/theme.ts").trim(),
      'export { t, tint } from "../../webview/theme";',
      "Devices & Connections must use the shared React style token module"
    );
  });

  test("React Flow canvas controls use the shared Devices & Connections treatment", () => {
    const themeCss = readSrc("webview/theme.css");
    for (const selector of [
      ".react-flow__controls",
      ".react-flow__controls button",
      ".react-flow__controls button:hover",
    ]) {
      assert.ok(
        themeCss.includes(selector),
        `shared webview theme must define ${selector} for canvas navigation chrome`
      );
    }

    const localControlCss = [
      "sfc/webview/sfcEditor.css",
      "statechart/webview/index.html",
      "statechart/stateChartEditor.ts",
    ];
    for (const file of localControlCss) {
      const src = readSrc(file);
      assert.ok(
        !src.includes("--vscode-button-background") &&
          !src.includes("--vscode-button-hoverBackground"),
        `${file} must not restyle canvas controls as primary buttons`
      );
      assert.ok(
        !/\\.react-flow__controls\\s*\\{/.test(src) &&
          !/\\.react-flow__controls button/.test(src),
        `${file} must not keep a private React Flow controls theme; use src/webview/theme.css`
      );
    }
  });

  test("Devices & Connections refits when endpoint children appear after managed Start", () => {
    const src = readSrc("networkCanvas/webview/NetworkCanvasApp.tsx");
    assert.ok(
      src.includes("child endpoints") && src.includes(".map((n) => n.id)"),
      "canvas fit signature must include child endpoint node IDs, not only host IDs"
    );
    assert.ok(
      !src.includes(".filter((n) => !n.parentId)\n      .map((n) => n.id)"),
      "managed Start can add endpoints under an existing host; top-level-only fit signatures leave a blank-looking canvas"
    );
  });

  test("visual editor right panes use the shared product chrome, not private sidebars", () => {
    const themeCss = readSrc("webview/theme.css");
    for (const selector of [
      ".trust-inspector",
      ".trust-inspector__header",
      ".trust-section",
      ".trust-button",
      ".trust-input",
    ]) {
      assert.ok(
        themeCss.includes(selector),
        `shared webview theme must define ${selector} for product chrome`
      );
    }

    const editorShells = [
      "sfc/webview/SfcEditor.tsx",
      "statechart/webview/StateChartEditor.tsx",
      "ladder/webview/LadderEditor.tsx",
      "blockly/webview/BlocklyEditor.tsx",
    ];
    for (const file of editorShells) {
      const src = readSrc(file);
      assert.ok(
        src.includes("trust-inspector"),
        `${file} must render the same inspector/right-pane chrome as Devices & Connections`
      );
      assert.ok(
        src.includes("trust-inspector__title"),
        `${file} must render its right-pane heading with the shared primary inspector title treatment`
      );
      assert.ok(
        !src.includes(">Editor tools<") && !src.includes('"Editor tools"'),
        `${file} must not render a generic "Editor tools" right-pane title; use the surface name`
      );
      assert.ok(
        !/right-pane-view-title|blockly-right-pane-title/.test(src),
        `${file} must not use private right-pane title classes for the primary inspector heading`
      );
    }

    const expectedSurfaceTitles: Array<[string, string]> = [
      ["sfc/webview/SfcEditor.tsx", "SFC editor"],
      ["statechart/webview/StateChartEditor.tsx", "Statechart editor"],
      ["ladder/webview/LadderEditor.tsx", "Ladder editor"],
      ["blockly/webview/BlocklyEditor.tsx", "Blockly editor"],
    ];
    for (const [file, title] of expectedSurfaceTitles) {
      assert.ok(
        readSrc(file).includes(title),
        `${file} must use the product-surface title "${title}"`
      );
    }

    const productChromeFiles = [
      "sfc/webview/SfcToolsPanel.tsx",
      "sfc/webview/SfcCodePanel.tsx",
      "sfc/webview/sfcEditor.css",
      "statechart/webview/StatechartToolsPanel.tsx",
      "statechart/webview/PropertiesPanel.tsx",
      "statechart/webview/ActionMappingsPanel.tsx",
      "ladder/webview/styles.css",
      "blockly/webview/styles.css",
      "blockly/webview/blocklyTheme.css",
    ];
    const forbiddenPrivateChrome = [
      "vscode-button-secondaryBackground",
      "vscode-button-secondaryHoverBackground",
      "vscode-button-secondaryForeground",
      "vscode-sideBar-background",
      "vscode-sideBarSectionHeader-background",
      "vscode-panel-border, #2b2b2b",
    ];
    for (const file of productChromeFiles) {
      const src = readSrc(file);
      for (const forbidden of forbiddenPrivateChrome) {
        assert.ok(
          !src.includes(forbidden),
          `${file} must not define private visual-editor chrome with ${forbidden}; use shared --trust-* product tokens/classes`
        );
      }
    }

    const forbiddenVisualPanelSelectors = [
      ".ladder-tools-panel__title",
      ".ladder-tools-panel__hint",
      ".ladder-tools-panel__section-title",
      ".ladder-tools-panel__grid",
      ".ladder-tools-panel__rungs",
      ".ladder-tools-panel__button",
      ".blockly-tools-panel",
      ".blockly-tools-panel__title",
      ".blockly-tools-panel__hint",
      ".blockly-tools-panel__grid",
      ".blockly-tools-panel__button",
    ];
    for (const [file, src] of [
      ["ladder/webview/styles.css", readSrc("ladder/webview/styles.css")],
      ["blockly/webview/styles.css", readSrc("blockly/webview/styles.css")],
    ] as const) {
      for (const selector of forbiddenVisualPanelSelectors) {
        assert.ok(
          !src.includes(selector),
          `${file} must not define private product chrome selector ${selector}; use shared trust-section/trust-button classes`
        );
      }
    }
  });

  test("Devices & Connections add pane uses the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/AddPane.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section"',
      'className="trust-input"',
    ]) {
      assert.ok(
        src.includes(required),
        `AddPane must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of [
      "--vscode-foreground",
      "--vscode-descriptionForeground",
      "--vscode-editorWidget-border",
      "--vscode-editorHoverWidget-background",
      "--vscode-input-background",
      "--vscode-input-border",
    ]) {
      assert.ok(
        !src.includes(forbidden),
        `AddPane product chrome must use shared --trust-* tokens/classes, not ${forbidden}`
      );
    }
  });

  test("Devices & Connections node summaries use the shared product chrome baseline", () => {
    const src = readSrc("networkCanvas/webview/NodeInspector.tsx");
    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-inspector__eyebrow"',
      'className="trust-section trust-section--grow"',
      'className="trust-button"',
    ]) {
      assert.ok(
        src.includes(required),
        `Node summary must use shared product chrome: missing ${required}`
      );
    }

    for (const forbidden of ["primaryBtn", "secondaryBtn", "dangerBtn"]) {
      assert.ok(
        !src.includes(forbidden),
        `Node summary must not keep a parallel inline button style via ${forbidden}`
      );
    }
  });

  test("protocol add/edit forms use the shared product chrome baseline", () => {
    const addPanel = readSrc("networkCanvas/webview/AddDevicePanel.tsx");
    const schemaFields = readSrc("networkCanvas/webview/SchemaFields.tsx");

    for (const required of [
      'className="trust-inspector"',
      'className="trust-inspector__header"',
      'className="trust-inspector__title"',
      'className="trust-section trust-section--grow"',
      'className="trust-field"',
      'className="trust-input"',
      'className="trust-button trust-button--primary"',
      "trust-message",
    ]) {
      assert.ok(
        addPanel.includes(required),
        `AddDevicePanel must use shared product chrome: missing ${required}`
      );
    }

    for (const required of [
      'className="trust-field"',
      "trust-input",
      "trust-input--error",
      "trust-field__message",
      "trust-field__message--error",
    ]) {
      assert.ok(
        schemaFields.includes(required),
        `SchemaFields must use shared product form chrome: missing ${required}`
      );
    }

    const files = new Map([
      ["AddDevicePanel", addPanel],
      ["SchemaFields", schemaFields],
    ]);
    for (const [name, src] of files) {
      for (const forbidden of [
        "--vscode-foreground",
        "--vscode-descriptionForeground",
        "--vscode-editorWidget-border",
        "--vscode-editorHoverWidget-background",
        "--vscode-input-background",
        "--vscode-input-border",
        "--vscode-errorForeground",
        "labelStyle",
        "inputStyle",
        "primaryBtn",
        "secondaryBtn",
      ]) {
        assert.ok(
          !src.includes(forbidden),
          `${name} must not keep a parallel protocol-form chrome via ${forbidden}; use shared trust-* classes`
        );
      }
    }
  });

  test("SFC toolbar add actions reframe the canvas so the result is visible", () => {
    const src = readSrc("sfc/webview/SfcEditor.tsx");
    const hook = readSrc("sfc/webview/hooks/useSfc.ts");
    assert.ok(
      src.includes("requestFitView") &&
        /useEffect\([\s\S]*reactFlowInstance\.fitView/.test(src),
      "SFC toolbar Add actions must request a committed fitView after mutating the graph"
    );
    assert.ok(
      /const handleAddStep[\s\S]*addNodeAtPosition\("step"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Add Step must reframe after adding so the new step is visible"
    );
    assert.ok(
      /const handleAddParallelSplit[\s\S]*addNodeAtPosition\("parallelSplit"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Split must reframe after adding so the new node is visible"
    );
    assert.ok(
      /const handleAddParallelJoin[\s\S]*addNodeAtPosition\("parallelJoin"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Join must reframe after adding so the new node is visible"
    );
    assert.ok(
      hook.includes("nextNodePosition"),
      "SFC hook must calculate toolbar-added node placement from existing node positions"
    );
    assert.ok(
      !hook.includes("150 + nds.length * 100"),
      "SFC toolbar-added nodes must not use the old overlapping vertical placement formula"
    );
  });

  test("SFC transition routing avoids stacking non-linear labels through the center line", () => {
    const stepNode = readSrc("sfc/webview/StepNode.tsx");
    const hook = readSrc("sfc/webview/hooks/useSfc.ts");
    const editor = readSrc("sfc/webview/SfcEditor.tsx");
    const transitionEdge = readSrc("sfc/webview/TransitionEdge.tsx");
    assert.ok(
      editor.includes("TransitionEdge") && editor.includes("edgeTypes"),
      "SFC must use the custom transition edge renderer, not React Flow's default midpoint labels"
    );
    assert.ok(
      transitionEdge.includes("EdgeLabelRenderer") &&
        transitionEdge.includes("labelOffset(sourcePosition)") &&
        transitionEdge.includes("sfc-transition-label"),
      "SFC transition labels must be offset from side-routed edges and inspectable by the VIS runner"
    );
    for (const handle of [
      "STEP_TARGET_LEFT",
      "STEP_TARGET_RIGHT",
      "STEP_SOURCE_LEFT",
      "STEP_SOURCE_RIGHT",
    ]) {
      assert.ok(
        stepNode.includes(handle),
        `SFC step nodes must expose ${handle} for readable side-routed transitions`
      );
      assert.ok(
        hook.includes(handle),
        `SFC import/connect routing must use ${handle} when a transition is not a simple downward edge`
      );
    }
    assert.ok(
      hook.includes("stepConnectionHandles"),
      "SFC edge routing must use the shared stepConnectionHandles helper"
    );
    assert.ok(
      /deltaY\s*<\s*0[\s\S]*STEP_SOURCE_LEFT[\s\S]*STEP_TARGET_LEFT/.test(hook),
      "backward SFC transitions must route to side handles instead of overlapping the vertical path"
    );
    assert.ok(
      /deltaY\s*>\s*expectedVerticalGap[\s\S]*STEP_SOURCE_RIGHT[\s\S]*STEP_TARGET_RIGHT/.test(hook),
      "skip SFC transitions must route to side handles instead of overlapping intermediate labels"
    );
  });

  test("Statechart import and add actions reframe the canvas inside the shared editor shell", () => {
    const src = readSrc("statechart/webview/StateChartEditor.tsx");
    const hook = readSrc("statechart/webview/hooks/useStateChart.ts");
    assert.ok(
      src.includes("STATECHART_FIT_VIEW_OPTIONS"),
      "Statechart editor must use explicit fitView options for predictable framing"
    );
    assert.ok(
      src.includes("requestFitView"),
      "Statechart editor must request fitView after graph mutations"
    );
    assert.ok(
      /importFromXState\(config\);[\s\S]*requestFitView\(\);/.test(src),
      "Statechart import must reframe after loading nodes"
    );
    assert.ok(
      /const handleAddState[\s\S]*addNewState\("normal"\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Add State must reframe so the new state and existing small graph stay visible"
    );
    assert.ok(
      /const handleAutoLayout[\s\S]*autoLayout\(\);[\s\S]*requestFitView\(\);/.test(
        src
      ),
      "Auto Layout must reframe after moving nodes"
    );
    assert.ok(
      /const STATE_GRID_X = 2[2-9]0;/.test(hook) &&
        /const STATE_GRID_Y = 2[2-9]0;/.test(hook),
      "Statechart grid spacing must leave room for edge labels between cards"
    );
    assert.ok(
      hook.includes("transitionHandles") &&
        hook.includes("STATE_SOURCE_RIGHT") &&
        hook.includes("STATE_TARGET_LEFT"),
      "Statechart same-row transitions must use side handles so labels do not sit on cards"
    );
    const edge = readSrc("statechart/webview/StateTransitionEdge.tsx");
    assert.ok(
      edge.includes("EdgeLabelRenderer") &&
        edge.includes("statechart-transition-label") &&
        edge.includes("labelTranslateY") &&
        edge.includes("sourcePosition === Position.Left") &&
        edge.includes("targetPosition === Position.Right") &&
        edge.includes("sourcePosition === Position.Bottom") &&
        edge.includes("targetPosition === Position.Top"),
      "Statechart backward and row-crossing transitions must lift labels away from cards"
    );
  });

  test("visual-editor chrome does not add private hardcoded colours", () => {
    const allowedSharedThemeFiles = new Set(["webview/theme.ts", "webview/theme.css"]);
    const filesToCheck = [
      "sfc/webview/SfcEditor.tsx",
      "sfc/webview/StepNode.tsx",
      "sfc/webview/sfcEditor.css",
      "statechart/webview/StateChartEditor.tsx",
      "statechart/webview/StateNode.tsx",
      "statechart/webview/StateTransitionEdge.tsx",
      "ladder/webview/LadderEditor.tsx",
      "ladder/webview/nodeDrawing.ts",
      "ladder/webview/styles.css",
      "blockly/webview/BlocklyEditor.tsx",
      "blockly/webview/ToolboxPanel.tsx",
      "blockly/webview/styles.css",
      "blockly/webview/blocklyTheme.css",
    ];
    const hardcodedColor = /#[0-9a-fA-F]{3,8}|rgba?\(/;
    for (const file of filesToCheck) {
      if (allowedSharedThemeFiles.has(file)) {
        continue;
      }
      const src = readSrc(file)
        .split("\n")
        .filter((line) => !line.includes("color-mix("))
        .join("\n");
      assert.ok(
        !hardcodedColor.test(src),
        `${file} must use shared --trust-* or t.* tokens for product chrome/semantic colours`
      );
    }
  });

  test("dead execution panels with embedded runtime controls are removed", () => {
    for (const file of [
      "sfc/webview/SfcExecutionPanel.tsx",
      "statechart/webview/ExecutionPanel.tsx",
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "src", file)),
        `${file} must not remain as dead duplicate runtime UI`
      );
    }
  });

  test("visual editor parse errors use user-facing recovery language", () => {
    for (const file of [
      "sfc/sfcEditor.ts",
      "statechart/stateChartEditor.ts",
      "blockly/blocklyEditor.ts",
    ]) {
      const src = readSrc(file);
      assert.ok(
        !/Editor Error:/.test(src),
        `${file} must not show raw 'Editor Error' notifications`
      );
      assert.ok(
        /Could not open/.test(src),
        `${file} must tell the user the visual file could not be opened`
      );
    }
  });
});
