import {
  assert,
  path,
  CHECK_PROGRAM_COMMAND,
  summarizeCheck,
  loadPackageJson,
  readSrc,
  readSrcSet,
  commandTitles,
} from "./ux-shell-contract-fixtures";
import type {
  ConfigurationContribution,
} from "./ux-shell-contract-fixtures";

suite("Phase 8 — Compile (authoritative project validation)", () => {
  test("summarizeCheck: passed vs failed wording", () => {
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 3 }),
      "Compile passed — 3 sources, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: true, status: "ok", errors: 0, warnings: 0, issues: [], source_count: 1 }),
      "Compile passed — 1 source, no errors."
    );
    assert.strictEqual(
      summarizeCheck({ ok: false, status: "failed", errors: 2, warnings: 1, issues: [] }),
      "Compile failed — 2 errors, 1 warning."
    );
    assert.strictEqual(
      summarizeCheck(
        { ok: false, status: "failed", errors: 1, warnings: 0, issues: [] },
        { errors: 2, warnings: 0 }
      ),
      "Compile failed — 2 errors, 0 warnings."
    );
  });
  test("Compile is a fixed sidebar action plus palette escape hatch, not a Project bucket item", () => {
    assert.strictEqual(
      commandTitles(loadPackageJson()).get("trust-lsp.checkProgram"),
      "Compile"
    );
    const view = readSrcSet("trustHomeView.ts", "trustHomeWebview.ts");
    assert.ok(
      view.includes('id="compile"') &&
        view.includes("CHECK_PROGRAM_COMMAND") &&
        CHECK_PROGRAM_COMMAND === "trust-lsp.checkProgram",
      "the sidebar Compile control must invoke the backend check command"
    );
    assert.ok(
      !view.includes("projectActionsMenu") && !view.includes("truST — Project"),
      "the retired Project menu must not remain"
    );
  });
  test("project switching is not hidden in the sidebar project name", () => {
    const view = readSrc("trustHomeWebview.ts");
    assert.ok(view.includes('id="projectName"'), "the open project name is rendered");
    assert.ok(
      !view.includes('projectNameEl.addEventListener') &&
        !view.includes('type: "projectMenu"'),
      "the project name must not become a hidden Open/Create/Example dropdown"
    );
  });
  test("project-open sidebar renders the project name as an identity row", () => {
    const view = readSrc("trustHomeWebview.ts");
    for (const required of [
      'class="project-identity"',
      'title="Current truST project"',
      "codicon-root-folder-opened",
      "project-identity__icon",
      'id="projectName"',
      ".project-identity .project-name",
      "font-weight: 600",
    ]) {
      assert.ok(view.includes(required), `project identity row must include ${required}`);
    }
    assert.ok(
      !/project-identity[^{]*{[^}]*#[0-9a-fA-F]{3,8}/.test(view),
      "project identity row must use theme variables, not raw colors"
    );
  });
  test("truST sidebar title exposes only Settings as a visible icon; New diagram stays in overflow", () => {
    const viewTitle = loadPackageJson().contributes?.menus?.["view/title"] ?? [];
    const settings = viewTitle.find((item) => item.command === "trust-lsp.openSettings");
    const newDiagram = viewTitle.find((item) => item.command === "trust-lsp.visual.newDiagram");
    assert.ok(settings, "Settings must be available from the truST view title");
    assert.strictEqual(settings?.group, "navigation@1", "Settings is the single visible view-title icon");
    assert.ok(newDiagram, "New diagram remains available as a secondary action");
    assert.ok(
      !String(newDiagram?.group || "").startsWith("navigation"),
      "New diagram must stay in the view-title overflow, not as an unexplained second icon"
    );
  });
  test("native Settings contribution uses product-language setting keys and titles", () => {
    const configuration = loadPackageJson().contributes
      ?.configuration as ConfigurationContribution;
    assert.strictEqual(configuration.title, "truST");
    const properties = configuration.properties ?? {};
    assert.ok(Object.keys(properties).length > 0, "expected contributed settings");
    for (const [key, property] of Object.entries(properties)) {
      assert.ok(
        !key.startsWith("trust-lsp."),
        `${key} must not render under the backend Trust-lsp heading in native Settings`
      );
      assert.ok(
        key.startsWith("trust."),
        `${key} must use the product-language trust.* Settings prefix`
      );
      assert.ok(property.title, `${key} must define a user-facing title`);
      assert.ok(
        !/trust-lsp/i.test(property.title ?? ""),
        `${key} title must not expose the internal trust-lsp setting prefix`
      );
    }
    assert.strictEqual(properties["trust.runtime.executablePath"]?.title, "Runtime executable path");
    assert.strictEqual(properties["trust.debugAdapter.executablePath"]?.title, "Debug adapter path");
    assert.strictEqual(properties["trust.testRunner.executablePath"]?.title, "Test runner path");
  });
  test("action row has a real narrow-width collapse rule", () => {
    const view = readSrc("trustHomeWebview.ts");
    assert.ok(
      /@media\s*\(max-width:\s*245px\)\s*\{[\s\S]*\.action-button \.label\s*\{\s*display:\s*none;\s*\}/.test(view),
      "action labels must collapse below the sidebar width threshold instead of wrapping or clipping"
    );
    assert.ok(
      /@media\s*\(max-width:\s*245px\)\s*\{[\s\S]*\.action-button\s*\{[\s\S]*min-height:\s*32px/.test(view),
      "action buttons must tighten vertically in the narrow sidebar state"
    );
  });
});
