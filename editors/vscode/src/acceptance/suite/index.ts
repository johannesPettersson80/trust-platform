import Mocha from "mocha";
import * as path from "path";

/** Runs the journey that the runner names in E2E_JOURNEY, one step after another in the order written. */
export function run(): Promise<void> {
  const journey = process.env.E2E_JOURNEY ?? "";
  if (!/^[a-z0-9-]+$/.test(journey)) {
    return Promise.reject(new Error(`E2E_JOURNEY names no journey: "${journey}"`));
  }
  const mocha = new Mocha({ ui: "tdd", color: false, reporter: "spec", timeout: Number(process.env.E2E_TIMEOUT_MS || 1_800_000) });
  mocha.addFile(path.join(__dirname, "journeys", `${journey}.js`));
  return new Promise((resolve, reject) => {
    mocha.run((failures) => (failures ? reject(new Error(`${failures} journey test failed`)) : resolve()));
  });
}
