import path from "node:path";
import { defineConfig } from "@playwright/test";
import { repoRoot } from "./lib/paths.mjs";

const scriptRoot = path.join(repoRoot, "scripts/captures/browser");
const reuseExistingServer = process.env.TRUST_CAPTURE_REUSE_EXISTING_SERVER === "1";

export default defineConfig({
  testDir: path.join(repoRoot, "scripts/captures/browser"),
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  timeout: 120_000,
  reporter: [["list"]],
  use: {
    browserName: "chromium",
    viewport: { width: 1280, height: 720 }
  },
  webServer: [
    {
      command: path.join(scriptRoot, "start-ide-server.sh"),
      url: "http://127.0.0.1:18080/ide",
      reuseExistingServer,
      timeout: 180_000
    },
    {
      command: path.join(scriptRoot, "start-hmi-server.sh"),
      url: "http://127.0.0.1:18082/hmi",
      reuseExistingServer,
      timeout: 180_000
    }
  ]
});
