import path from "node:path";
import { defineConfig } from "@playwright/test";
import { repoRoot } from "./lib/paths.mjs";

const reuseExistingServer = process.env.TRUST_CAPTURE_REUSE_EXISTING_SERVER === "1";

export default defineConfig({
  testDir: path.join(repoRoot, "scripts/captures/vscode"),
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  timeout: 180_000,
  reporter: [["list"]],
  use: {
    browserName: "chromium",
    viewport: { width: 1440, height: 900 }
  },
  webServer: {
    command: path.join(repoRoot, "scripts/captures/vscode/start-code-server.sh"),
    url: "http://127.0.0.1:8080",
    reuseExistingServer,
    timeout: 240_000
  }
});
