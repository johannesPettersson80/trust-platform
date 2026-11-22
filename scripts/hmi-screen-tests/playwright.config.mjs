// Screen tests of the HMI (specification 39, §18 phase 3; §19.2 Browser): the operator journey runs
// against the tank-level example in simulation, tutorial 24's alarm journey against a tank-level
// runtime of its own, room heating's Try it steps 1 and 4 against that example, the role locks
// against the equipment example; the
// reference page compares the drawing with its approved screenshots. CHROMIUM may name a browser of
// its own.
import { defineConfig } from "@playwright/test";

const WEB_PORT = Number(process.env.HMI_SCREEN_WEB_PORT || 18480);
export const EQUIPMENT_PORT = Number(process.env.HMI_SCREEN_EQUIPMENT_PORT || 18481);
const EQUIPMENT_CONTROL_PORT = Number(process.env.HMI_SCREEN_EQUIPMENT_CONTROL_PORT || 19481);
const RANGES_PORT = Number(process.env.HMI_SCREEN_RANGES_PORT || 18482);
const RANGES_CONTROL_PORT = Number(process.env.HMI_SCREEN_RANGES_CONTROL_PORT || 19482);
const TUTORIAL_PORT = Number(process.env.HMI_SCREEN_TUTORIAL_PORT || 18485);
const TUTORIAL_CONTROL_PORT = Number(process.env.HMI_SCREEN_TUTORIAL_CONTROL_PORT || 19485);
const ROOM_HEATING_PORT = Number(process.env.HMI_SCREEN_ROOM_HEATING_PORT || 18486);
const ROOM_HEATING_CONTROL_PORT = Number(process.env.HMI_SCREEN_ROOM_HEATING_CONTROL_PORT || 19486);

export default defineConfig({
  testDir: "tests",
  timeout: 180_000,
  retries: 0,
  workers: 1,
  reporter: [["list"], ["json", { outputFile: "test-results/results.json" }]],
  snapshotPathTemplate: "{testDir}/screens/{arg}{ext}",
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.002, threshold: 0.2, animations: "disabled" } },
  use: {
    locale: "en-US",
    timezoneId: "UTC",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    baseURL: `http://127.0.0.1:${WEB_PORT}`,
    viewport: { width: 1920, height: 1080 },
    launchOptions: process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM, args: ["--no-sandbox"] } : {},
  },
  webServer: [
    {
      command: "node serve-example.mjs plant-live 18484 19484",
      url: "http://127.0.0.1:18484/hmi/plant-live",
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      command: `node serve-example.mjs hmi_equipment ${Number(process.env.HMI_SCREEN_PERFORMANCE_PORT || 18483)} 19483 performance`,
      url: `http://127.0.0.1:${Number(process.env.HMI_SCREEN_PERFORMANCE_PORT || 18483)}/hmi/performance`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      command: `node serve-example.mjs hmi_equipment ${RANGES_PORT} ${RANGES_CONTROL_PORT} equipment-ranges`,
      url: `http://127.0.0.1:${RANGES_PORT}/hmi/equipment-ranges`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      command: "node serve-example.mjs",
      url: `http://127.0.0.1:${WEB_PORT}/hmi/overview`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      command: `node serve-example.mjs hmi_equipment ${EQUIPMENT_PORT} ${EQUIPMENT_CONTROL_PORT}`,
      url: `http://127.0.0.1:${EQUIPMENT_PORT}/hmi/overview`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      // Tutorial 24's journey trips and resets the tank, so it runs on a runtime the operator journey
      // does not share.
      command: `node serve-example.mjs hmi_tank_level ${TUTORIAL_PORT} ${TUTORIAL_CONTROL_PORT}`,
      url: `http://127.0.0.1:${TUTORIAL_PORT}/hmi/overview`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
    {
      // Room heating's Try it steps start, trip and recover the plant on a runtime of their own.
      command: `node serve-example.mjs room_heating ${ROOM_HEATING_PORT} ${ROOM_HEATING_CONTROL_PORT}`,
      url: `http://127.0.0.1:${ROOM_HEATING_PORT}/hmi/overview`,
      timeout: 240_000,
      reuseExistingServer: false,
    },
  ],
});
