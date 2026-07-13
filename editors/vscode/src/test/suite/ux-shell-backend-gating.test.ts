import {
  assert,
  loadPackageJson,
  readSrc,
  readSrcSet,
  readIoPanelDocumentSource,
  commandTitles,
} from "./ux-shell-contract-fixtures";

suite("Phases 8–10 — honest backend gating (no fakes, no dead buttons)", () => {
  test("unsupported Deploy is absent instead of occupying the primary run surface", () => {
    for (const [command, title] of commandTitles(loadPackageJson())) {
      assert.ok(
        !/send to plc|deploy to/i.test(title),
        `${command} must not expose a deploy action before the backend exists`
      );
    }
    const sidebar = readSrcSet("trustHomeView.ts", "trustHomeWebview.ts");
    assert.ok(
      !sidebar.includes('id="deploy"') &&
        !sidebar.includes('case "deploy"') &&
        !sidebar.includes("Deploy is not available for this target yet."),
      "the simplified sidebar must not advertise a dead Deploy action"
    );
    assert.ok(!/send to plc/i.test(sidebar), "the sidebar must not use the old Send to PLC wording");
  });
  test("Compile state uses icon + token role, and clean compile settles to neutral", () => {
    // Ignore comments — only code/UI strings count.
    const code = readSrc("trustHomePresentation.ts")
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");
    assert.ok(
      code.includes('case "clean"') &&
        code.includes('icon: "codicon-check"') &&
        code.includes('tone: "neutral"') &&
        code.includes('variant: "outline"'),
      "a clean compile must show a check icon in the neutral outlined Compile button, not a persistent green button"
    );
    assert.ok(
      code.includes('case "dirty"') &&
        code.includes('icon: "codicon-warning"') &&
        code.includes('tone: "warning"'),
      "dirty state must be icon + warning token, not color alone"
    );
    assert.ok(
      code.includes('icon: "codicon-error"') &&
        code.includes('tone: "danger"'),
      "compile failures must be icon + danger token, not color alone"
    );
    assert.ok(
      !/build ok|build succeeded|build successful/i.test(code),
      "must NOT claim an authoritative build from sidebar diagnostics"
    );
  });
  test("sidebar two-button state table is explicit and has one primary source of truth", () => {
    const presentation = readSrc("trustHomePresentation.ts");
    const webview = readSrc("trustHomeWebview.ts");
    for (const fn of ["compileButtonState", "runtimeActionButtonState"]) {
      assert.ok(
        presentation.includes(`function ${fn}`),
        `${fn} must own one sidebar button state table`
      );
    }
    assert.ok(
      presentation.includes('case "start"') &&
        presentation.includes('case "connect"') &&
        presentation.includes('variant: enabled ? "filled" : "outline"') &&
        presentation.includes('tone: enabled ? "primary" : "disabled"'),
      "Start/Connect are the only runtime actions that become filled primary buttons"
    );
    assert.ok(
      presentation.includes('case "stop"') &&
        presentation.includes('case "disconnect"') &&
        presentation.includes('tone: "neutral"') &&
        presentation.includes('variant: "outline"'),
      "Stop/Disconnect must stay neutral outlined routine actions"
    );
    assert.ok(
      webview.includes("setButton(compileEl") &&
        webview.includes("setButton(actionEl") &&
        !webview.includes("setButton(debugEl") &&
        !webview.includes("setButton(deployEl"),
      "the Compile and one lifecycle action must be projected from typed button-state objects without duplicate Debug or Deploy controls"
    );
    assert.ok(
      !webview.includes("🐞") &&
        !webview.includes("⚒") &&
        !webview.includes("⤓") &&
        !webview.includes("▶"),
      "the two-button row must not use emoji/text glyphs; Codicons carry the shape"
    );
  });
  test("Live Values does not show stale compile diagnostics before a real result", () => {
    const html = readIoPanelDocumentSource();
    const script = readSrc("ioPanel.webview.js");
    assert.ok(
      !html.includes("Compile Diagnostics"),
      "Live Values must not show the old Runtime Panel compile-diagnostics card"
    );
    assert.ok(
      !html.includes("No compile run yet") && !script.includes("No compile run yet"),
      "Live Values must not contradict Compile with a stale no-compile state"
    );
    assert.ok(
      /id="diagnostics"[^>]*display:none/.test(html),
      "diagnostics details stay hidden until a real compile/reload result exists"
    );
  });
  test("managed local runtimes are projected into the sidebar Target from the fleet lifecycle", () => {
    const src = readSrc("trustHomeView.ts");
    // The sidebar lists real managed runtimes + drives Start/Stop
    // through the fleet lifecycle — no fake static "Local runtime" entry, no false advertising.
    assert.ok(
      src.includes("listManagedRuntimes"),
      "the sidebar must list managed runtimes from the fleet lifecycle"
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
      helper.includes("runtimeLifecycleService.connectRemote(") &&
        helper.includes("result.controlEndpoint") &&
        helper.includes("managedRuntimeLabel(name)") &&
        helper.includes("setSelectedRuntimeId(name)"),
      "the shared managed-runtime attach helper must attach to the reached endpoint and set the Target"
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
