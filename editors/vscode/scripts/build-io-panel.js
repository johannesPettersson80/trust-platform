const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const files = [
  ["src/ioPanelAdsRows.webview.js", "media/ioPanelAdsRows.js"],
  ["src/ioPanel.webview.js", "media/ioPanel.js"],
];

for (const [sourceRelative, destRelative] of files) {
  const sourcePath = path.join(root, sourceRelative);
  const destPath = path.join(root, destRelative);
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Missing source webview script: ${sourcePath}`);
  }
  const source = fs.readFileSync(sourcePath, "utf8");
  fs.mkdirSync(path.dirname(destPath), { recursive: true });
  fs.writeFileSync(destPath, source, "utf8");
  const lineCount = source.split(/\r?\n/).length - 1;
  console.log(`Wrote ${path.relative(root, destPath)} (${lineCount} lines)`);
}
