import {
  assert,
  fs,
  path,
  pickAuthToken,
  extensionRoot,
  readSrc,
  readIoPanelDocumentSource,
} from "./ux-shell-contract-fixtures";

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
    for (const file of ["runtimeTarget.ts", "runtimeOnlineConnection.ts", "io-panel/status.ts"]) {
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
    const liveValuesHost = readSrc("ioPanel.ts");
    const liveValuesHtml = readIoPanelDocumentSource();
    const liveValuesWebview = readSrc("ioPanel.webview.js");
    assert.ok(
      !liveValuesHost.includes("runtimeControlAuthToken") &&
        !liveValuesHost.includes('"runtime.controlAuthToken"') &&
        !liveValuesHtml.includes('id="runtimeControlAuthToken"') &&
        !liveValuesWebview.includes("runtimeControlAuthToken"),
      "Live Values must not expose or persist plaintext runtime tokens",
    );
  });
  test("legacy plaintext token setting is not contributed to native Settings", () => {
    const pkg = fs.readFileSync(path.join(extensionRoot(), "package.json"), "utf8");
    assert.ok(
      !pkg.includes("trust-lsp.runtime.controlAuthToken"),
      "legacy plaintext token setting remains code fallback only and must not appear in native Settings"
    );
    assert.ok(
      pkg.includes("trust.runtime.authTokenFallback") &&
        /legacy/i.test(pkg) &&
        /fallback/i.test(pkg) &&
        /secret store/i.test(pkg),
      "canonical token fallback setting must explain that SecretStorage is the normal path"
    );
  });
});
