import {
  assert,
  fs,
  path,
  extensionRoot,
  loadPackageJson,
  readSrc,
  readSrcSet,
  commandTitles,
} from "./ux-shell-contract-fixtures";

suite("S-24 — Libraries surface contract", () => {
  test("Libraries is reachable from the sidebar and the command palette escape hatch", () => {
    assert.strictEqual(
      commandTitles(loadPackageJson()).get("trust-lsp.libraries.open"),
      "Open Libraries"
    );
    const pkg = loadPackageJson();
    assert.ok(
      pkg.contributes?.commands?.some(
        (command) => command.command === "trust-lsp.libraries.open"
      ),
      "Libraries command must be contributed as the palette escape hatch"
    );
    const view = readSrcSet("trustHomeView.ts", "trustHomeWebview.ts");
    assert.ok(
      view.includes("trust-lsp.libraries.open") && view.includes('id="navLibraries"'),
      "the sidebar destination must invoke Libraries"
    );
    assert.ok(
      /Libraries/.test(view),
      "the sidebar destination must use the user-facing Libraries label"
    );
  });
  test("Libraries uses the shared truST theme instead of a private token layer", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('"src", "webview", "theme.css"'),
      "Libraries must load the shared theme.css token layer"
    );
    assert.ok(
      !source.includes("--trust-canvas:"),
      "Libraries must not redeclare shared --trust-* theme tokens"
    );
    assert.ok(
      source.includes("trust-button--primary"),
      "Libraries must use shared product button roles"
    );
    const ignore = fs.readFileSync(path.join(extensionRoot(), ".vscodeignore"), "utf8");
    assert.ok(
      ignore.includes("!src/webview/theme.css"),
      "packaged VSIX must include the shared theme.css file used by Libraries"
    );
  });
  test("curated libraries are packaged and catalog remains gated", () => {
    const root = extensionRoot();
    for (const library of ["oscat", "plcopen_motion"]) {
      assert.ok(
        fs.existsSync(path.join(root, "media", "libraries", library, "trust-lsp.toml")),
        `${library} curated library must be packaged under media/libraries`
      );
    }
    const source = readSrc("libraries.ts");
    assert.ok(
      !/From library catalog/i.test(source),
      "catalog source must stay hidden until the ST-package-install backend contract exists"
    );
    assert.ok(
      source.includes("No libraries added. Add OSCAT, PLCopen Motion, or your own."),
      "empty state must be plain-language and must not mention trust-lsp.toml"
    );
  });
  test("Libraries failures stay visible with a recovery action but clear after success", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('role="alert"') && source.includes("Fix and retry"),
      "failed add attempts must render an actionable in-surface error"
    );
    assert.ok(
      source.includes("if (result.ok || !result.message)"),
      "failed add attempts must not immediately clear the in-surface error"
    );
    assert.ok(
      source.includes("this.lastError = error;") && !source.includes("error || this.lastError"),
      "successful refreshes must clear stale library errors"
    );
  });
  test("curated library updates compare the vendored project copy", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes("const resolvedPath = curated ? manifestPath : projectInfo?.path ?? manifestPath"),
      "curated rows must use the project-vendored path, not a stale LSP project-info path"
    );
    assert.ok(
      source.includes("curated") && source.includes("packageVersion ?? dependency.version ?? projectInfo?.version"),
      "curated update state must prefer the vendored package version before project-info"
    );
  });
  test("library symbol counts use real singular/plural copy", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes('function countLabel(count: number, singular: string') &&
        source.includes('countLabel(symbols.length, "symbol")'),
      "Libraries must use a shared countLabel helper for symbol availability"
    );
    assert.ok(
      !source.includes('${symbols.length} symbols available'),
      "Libraries must not render awkward copy such as '1 symbols available'"
    );
    assert.ok(
      source.includes("countLabel((lib.symbols || []).length, 'symbol')") &&
        !source.includes("esc((lib.symbols || []).length) + ' symbols'"),
      "expanded library contents must also use singular/plural copy"
    );
  });
  test("library symbol browser supports search, pagination, detail, and insertion", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      !source.includes(".slice(0, 24)") && !source.includes("groupSymbols"),
      "Libraries must not truncate symbols into inert first-24 chips"
    );
    assert.ok(
      source.includes("data-symbol-search") &&
        source.includes("Search all ") &&
        source.includes("data-symbol-page") &&
        source.includes("openLibraries"),
      "Libraries must provide search within a library and page through the full symbol list without collapsing the row"
    );
    assert.ok(
      source.includes("data-symbol-select") &&
        source.includes("symbol-detail") &&
        source.includes("declarationText(symbol)"),
      "Libraries must show a per-symbol detail panel with declaration context"
    );
    assert.ok(
      source.includes("Insert declaration") &&
        source.includes("Copy snippet") &&
        source.includes("insertDeclarationText") &&
        source.includes("visibleTextEditors.find") &&
        source.includes("declarationInsertion") &&
        source.includes("VAR"),
      "Libraries must insert declaration snippets into a visible ST VAR block, even after the webview takes focus"
    );
  });
  test("library row actions use user-facing verbs and versioned updates", () => {
    const source = readSrc("libraries.ts");
    assert.ok(
      source.includes(">View source</button>") &&
        !source.includes(">Open source</button>"),
      "Libraries must use View source for read-only vendor/library files"
    );
    assert.ok(
      source.includes(">Update to ' + esc(lib.updateAvailable.next) + '</button>") &&
        !source.includes('">Update</button>'),
      "Library update buttons must name the target version"
    );
  });
});
