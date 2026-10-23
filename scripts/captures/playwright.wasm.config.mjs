import path from "node:path";
import { defineConfig } from "@playwright/test";
import { repoRoot } from "./lib/paths.mjs";

export default defineConfig({
  testDir: path.join(repoRoot, "scripts/captures/wasm"),
  workers: 1,
  retries: 0,
  timeout: 60_000,
  reporter: [["list"]],
  use: { browserName: "chromium", viewport: { width: 1280, height: 900 } },
  webServer: {
    command: "python3 -m http.server 14173 --bind 127.0.0.1 --directory target/browser-analysis-wasm",
    cwd: repoRoot,
    url: "http://127.0.0.1:14173/web/",
    reuseExistingServer: false,
  },
});
