import {
  assert,
  fs,
  path,
  extensionRoot,
  loadPackageJson,
  readSrc,
  paletteHidden,
  commandTitles,
  HIDDEN_FROM_PALETTE,
  RETIRED_COMMUNICATION_COMMANDS,
} from "./ux-shell-contract-fixtures";

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
  test("retired Communication and ADS panel commands are removed, not hidden escapes", () => {
    const pkg = loadPackageJson();
    const titles = commandTitles(pkg);
    const activationEvents = JSON.stringify(pkg.activationEvents ?? []);
    const menuSurface = JSON.stringify(pkg.contributes?.menus ?? {});
    for (const command of RETIRED_COMMUNICATION_COMMANDS) {
      assert.ok(!titles.has(command), `${command} must not remain a contributed command`);
      assert.ok(
        !activationEvents.includes(command),
        `${command} must not remain an activation event`
      );
      assert.ok(!menuSurface.includes(command), `${command} must not remain in menus`);
    }
  });
  test("hidden commands are still registered where the surface contract keeps them", () => {
    const titles = commandTitles(loadPackageJson());
    for (const command of HIDDEN_FROM_PALETTE) {
      assert.ok(
        titles.has(command),
        `${command} must remain a registered command (hidden from palette, not removed)`
      );
    }
  });
  test("retired 3D twin surface is absent from the VS Code product surface", () => {
    const retiredSlug = ["trust", "twin"].join("-");
    const retiredCamel = ["trust", "Twin"].join("");
    const retiredSnake = ["trust", "twin"].join("_");
    const retiredPattern = new RegExp(
      `${retiredSlug}|${retiredCamel}|${retiredSnake}`,
      "i"
    );
    const pkg = loadPackageJson();
    const packageSurface = JSON.stringify({
      activationEvents: pkg.activationEvents,
      commands: pkg.contributes?.commands,
      languageModelTools: pkg.contributes?.languageModelTools,
      menus: pkg.contributes?.menus,
      scripts: pkg.scripts,
    });
    assert.ok(
      !retiredPattern.test(packageSurface),
      "retired 3D twin surface must not be contributed as an activation event, command, LM tool, menu, or build script"
    );
    for (const removedFile of [
      `${retiredCamel}Panel.ts`,
      path.join("lm-tools", `${retiredCamel}Tools.ts`),
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "src", removedFile)),
        `${removedFile} must not remain in the VS Code source tree`
      );
    }
    for (const removedOutput of [
      `${retiredCamel}Panel.js`,
      `${retiredCamel}Panel.js.map`,
      path.join("lm-tools", `${retiredCamel}Tools.js`),
      path.join("lm-tools", `${retiredCamel}Tools.js.map`),
    ]) {
      assert.ok(
        !fs.existsSync(path.join(extensionRoot(), "out", removedOutput)),
        `${removedOutput} must not remain in the packaged VS Code output tree`
      );
    }
    assert.ok(
      !fs.existsSync(path.join(extensionRoot(), "media", retiredSlug)),
      "retired 3D twin media assets must not be packaged by the extension"
    );
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
  test("advanced refactor commands use self-explanatory Structured Text wording", () => {
    const pkg = loadPackageJson();
    const command = (pkg.contributes?.commands ?? []).find(
      (cmd: { command?: string }) => cmd.command === "trust-lsp.moveNamespace.ui"
    );
    assert.strictEqual(
      command?.title,
      "Move Structured Text Namespace",
      "namespace refactor command must not appear as the ambiguous bare title 'Move Namespace'"
    );
    const source = readSrc("namespaceMove.ts");
    assert.ok(
      source.includes("Move Structured Text Namespace") && !source.includes('"Move Namespace"'),
      "the editor code action must use the same self-explanatory namespace wording"
    );
    assert.ok(
      source.includes("return_edit: true") &&
        source.includes("applyLspWorkspaceEdit") &&
        source.includes("ensureTargetFile") &&
        source.includes("removeCreatedTargetIfEmpty"),
      "the command must request an edit, apply it in VS Code, pre-create missing targets, and clean them up on failure"
    );
  });
  test("the Communication panel is no longer user-facing", () => {
    const pkg = loadPackageJson();
    const titles = commandTitles(pkg);
    assert.ok(
      !titles.has("trust-lsp.communication.openPanel"),
      "Communication must not remain a contributed command"
    );
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
