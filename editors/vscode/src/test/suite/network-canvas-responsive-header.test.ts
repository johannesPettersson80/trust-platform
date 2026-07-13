import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";

function extensionRoot(): string {
  return path.resolve(__dirname, "..", "..", "..");
}

function readSource(relativePath: string): string {
  return fs.readFileSync(path.join(extensionRoot(), "src", relativePath), "utf8");
}

suite("Devices & Connections responsive header", () => {
  test("keeps the one ADS discovery action fully usable in a 232px editor pane", () => {
    const header = readSource("networkCanvas/webview/NetworkCanvasHeader.tsx");
    const sharedTheme = readSource("webview/theme.css");

    assert.strictEqual(
      (header.match(/Discover ADS devices/g) ?? []).length,
      1,
      "the toolbar must keep exactly one ADS discovery action",
    );
    assert.match(
      header,
      /<header className="trust-network-header"[\s\S]*<div className="trust-network-header__actions">[\s\S]*<button\s+className="trust-network-header__discover"[\s\S]*>\s*Discover ADS devices\s*<\/button>/,
      "the primary ADS action must participate in the responsive shared header",
    );
    assert.match(
      sharedTheme,
      /@media \(max-width: 520px\)\s*{[\s\S]*\.trust-network-header\s*{[\s\S]*flex-wrap: wrap;[\s\S]*\.trust-network-header__actions\s*{[\s\S]*flex-wrap: wrap;[\s\S]*\.trust-network-header__discover\s*{[\s\S]*flex: 1 0 100%;[\s\S]*max-width: 100%;[\s\S]*width: 100%;/,
      "at the observed 232px pane width, the header must wrap and reserve a complete row for the primary action",
    );
    assert.doesNotMatch(
      sharedTheme,
      /\.trust-network-header__discover\s*{[^}]*(?:overflow:\s*hidden|text-overflow:\s*ellipsis)/s,
      "the primary action must not hide or ellipsize its label",
    );
    assert.doesNotMatch(
      sharedTheme,
      /\.trust-network-header(?:__\w+)?\s*{[^}]*\border\s*:/s,
      "the visual order must remain the DOM and keyboard focus order",
    );
    assert.ok(
      header.indexOf("Discover ADS devices") < header.indexOf(">\n          Filter\n"),
      "the primary discovery action must be first in its visual and keyboard action group",
    );
  });

  test("caps the discovery drawer and its reserved space to the visible pane", () => {
    const pane = readSource("networkCanvas/webview/DiscoverPane.tsx");
    const app = readSource("networkCanvas/webview/NetworkCanvasApp.tsx");

    assert.ok(
      pane.includes('width: "min(340px, 100%)"'),
      "the discovery drawer must never begin outside a narrow viewport",
    );
    assert.ok(
      app.includes("width: `min(${activeDrawerW}px, 100%)`"),
      "the drawer spacer must reserve no more than the visible viewport width",
    );
    assert.ok(
      pane.includes('overflowX: "hidden"') &&
        sharedThemeAllowsDiscoveryButtonsToWrap(readSource("webview/theme.css")),
      "narrow discovery content must not create a horizontal scrollbar",
    );
  });

  test("preserves the existing wide toolbar and shared VS Code theme", () => {
    const header = readSource("networkCanvas/webview/NetworkCanvasHeader.tsx");
    const sharedTheme = readSource("webview/theme.css");

    assert.match(
      sharedTheme,
      /\.trust-network-header\s*{[\s\S]*display: flex;[\s\S]*flex-wrap: nowrap;[\s\S]*gap: 12px;[\s\S]*padding: 10px 16px;/,
      "the normal-width toolbar must retain its single-row layout and spacing",
    );
    assert.ok(
      header.includes("background: t.surface") &&
        header.includes("borderBottom: `1px solid ${t.border}`") &&
        header.includes('style={toolbarButtonStyle(discoverActive, "primary")}'),
      "the responsive layout must retain the shared themed header and primary-button styling",
    );
    assert.ok(
      !fs.existsSync(
        path.join(extensionRoot(), "src", "networkCanvas", "webview", "theme.css"),
      ),
      "Devices & Connections must not introduce a parallel theme file",
    );
  });
});

function sharedThemeAllowsDiscoveryButtonsToWrap(theme: string): boolean {
  return /\[data-role="ads-discovery-section"\] \.trust-button\s*{[\s\S]*overflow-wrap: anywhere;[\s\S]*white-space: normal;/.test(
    theme,
  );
}
