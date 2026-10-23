// Transport only: Rust owns browser deadlines, diagnostics and subprocess cleanup.
const { execFileSync } = require("child_process");
const path = require("path");

const REPO = path.resolve(__dirname, "../../..");
let binary;

function runBrowser(stage, executable, args) {
  if (!binary) {
    const metadata = JSON.parse(execFileSync(
      "cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"],
      { cwd: REPO, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] }
    ));
    binary = path.join(metadata.target_directory, "debug", process.platform === "win32" ? "xtask.exe" : "xtask");
  }
  return execFileSync(binary, ["browser-command", stage, executable, ...args], {
    cwd: REPO, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"],
    maxBuffer: 64 * 1024 * 1024,
  });
}

module.exports = { runBrowser };
