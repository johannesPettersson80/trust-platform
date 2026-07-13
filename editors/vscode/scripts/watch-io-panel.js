const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const files = [
  ["src/ioPanelAdsRows.webview.js", "media/ioPanelAdsRows.js"],
  ["src/ioPanel.webview.js", "media/ioPanel.js"],
];

let lastOutOfSync = null;

function readFileSafe(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch {
    return null;
  }
}

function warnOutOfSync(message) {
  console.warn(`\n[panel] ${message}\n`);
}

function checkSync() {
  const outOfSync = files.some(([sourceRelative, destRelative]) => {
    const sourcePath = path.join(root, sourceRelative);
    const destPath = path.join(root, destRelative);
    const source = readFileSafe(sourcePath);
    const dest = readFileSafe(destPath);
    if (!source) {
      warnOutOfSync(
        `Missing ${path.relative(root, sourcePath)}. Cannot verify panel script.`
      );
      return true;
    }
    return !dest || source !== dest;
  });
  if (lastOutOfSync === null || outOfSync !== lastOutOfSync) {
    if (outOfSync) {
      warnOutOfSync(
        "Live Values source scripts differ from media output. Run `npm run build:panel`."
      );
    } else {
      console.log("[panel] ioPanel.js is in sync.");
    }
  }
  lastOutOfSync = outOfSync;
}

checkSync();
const interval = setInterval(checkSync, 2000);

const tsc = spawn("tsc", ["-watch", "-p", "./"], { stdio: "inherit" });

function shutdown(code) {
  clearInterval(interval);
  process.exit(code ?? 0);
}

tsc.on("close", shutdown);
tsc.on("error", (err) => {
  console.error("[panel] Failed to start tsc:", err);
  shutdown(1);
});

process.on("SIGINT", () => tsc.kill("SIGINT"));
process.on("SIGTERM", () => tsc.kill("SIGTERM"));
