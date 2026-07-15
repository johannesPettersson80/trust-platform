const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const sourcePath = path.join(root, "src", "ioPanel.webview.js");
const destPath = path.join(root, "media", "ioPanel.js");

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
  const source = readFileSafe(sourcePath);
  const dest = readFileSafe(destPath);
  if (!source) {
    warnOutOfSync(
      `Missing ${path.relative(root, sourcePath)}. Cannot verify panel script.`
    );
    lastOutOfSync = true;
    return;
  }
  const outOfSync = !dest || source !== dest;
  if (lastOutOfSync === null || outOfSync !== lastOutOfSync) {
    if (outOfSync) {
      warnOutOfSync(
        "ioPanel.webview.js differs from media/ioPanel.js. Run `npm run build:panel`."
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
