// The steps that the Control Studio journeys share: starting the Simulator from the side panel, opening Control Studio,
// identifying the plant, choosing PI and its requirements, tuning and validating (specification 35, B3 to B6).
import * as cdp from "./cdp";
import { expect, openScreen, pressHome, Session, setValue, waitText } from "./session";

/** A script that gives the text of the element `selector` while it is shown, or nothing. */
export const visible = (selector: string) =>
  `var e=d.querySelector(${JSON.stringify(selector)});return e&&!e.hidden?e.innerText:'';`;

/** A script that gives the message of the Control Studio page `page` (model, tune or verify). */
export const message = (page: string) => visible(`[data-studio-message="${page}"]`);

/** The requirements of the commissioning walkthrough: a 1 °C test setpoint change, at most 0.1 °C steady-state error and a
 * ±0.05 °C settling band, with the settling time and the validation's duration of each journey. */
export function requirements(settling: string, duration: string): Record<string, string> {
  return { referenceChange: "1", maximumError: "0.1", settlingTimeSeconds: settling, settlingBand: "0.05", durationSeconds: duration };
}

const statusRunning = (status: string[]) => (status || []).some((item) => /Simulator running/i.test(item));

/** Presses Start in the side panel and waits until the status bar says the Simulator runs. */
export async function startSimulator(s: Session): Promise<{ clicked: string; status: string[] }> {
  const clicked = await pressHome(s, "^Start$");
  const status = await cdp.pollValue("the Simulator running", () => cdp.statusBar(s.wb), statusRunning, 180_000);
  expect(statusRunning(status), `the status bar says the Simulator runs: ${JSON.stringify(status)}`);
  return { clicked, status };
}

/** Opens Control Studio from the side panel; gives its webview once it offers Identify plant. */
export async function openStudio(s: Session): Promise<string> {
  const studio = await openScreen(s, "Control Studio", () => pressHome(s, "Control Studio"), 60_000);
  await waitText(s, studio, "Identify plant", 60_000);
  return studio;
}

/** Shows the Control Studio page `page` (model, tune or verify). */
export async function tab(s: Session, studio: string, page: string): Promise<void> {
  await cdp.evalWebview(s.wb, studio, `d.querySelector('[data-engineer-stage-tab="${page}"]').click();return 'ok';`);
  await cdp.sleep(1_500);
}

/** Presses the button `selector` of Control Studio, or fails the step when it is missing or disabled. */
export async function press(s: Session, studio: string, selector: string): Promise<void> {
  const done = await cdp.evalWebview(s.wb, studio,
    `var b=d.querySelector(${JSON.stringify(selector)});if(!b)return 'NONE';if(b.disabled)return 'DISABLED';b.click();return 'CLICKED';`);
  expect(done === "CLICKED", `${selector}: ${done}`);
}

/** Identifies the plant with the Automatic method; gives the model and the page's message. */
export async function identify(s: Session, studio: string): Promise<{ model: string; message: string }> {
  await press(s, studio, "[data-engineer-find]");
  const model = await cdp.pollValue("the model", () => cdp.evalWebview(s.wb, studio, visible("[data-studio-model-result]")), Boolean, 300_000);
  expect(/G\(s\)/.test(model), `Identify shows a model: ${model}`);
  return { model, message: await cdp.evalWebview(s.wb, studio, message("model")) };
}

/** Chooses PI on the Tune page and enters `values` as the requirements; gives what each field holds then. */
export async function enterRequirements(s: Session, studio: string, values: Record<string, string>): Promise<Record<string, string>> {
  await tab(s, studio, "tune");
  const pi = await setValue(s, studio, "[data-studio-controller]", "P1.Controller::PI");
  expect(pi === "P1.Controller::PI", `PI is chosen: ${pi}`);
  await press(s, studio, "[data-studio-requirements]");
  await cdp.sleep(1_500);
  const set: Record<string, string> = {};
  for (const [key, value] of Object.entries(values)) {
    set[key] = await setValue(s, studio, `[data-studio-requirement="${key}"]`, value);
  }
  await press(s, studio, "[data-studio-editor-done]");
  await cdp.sleep(1_500);
  return set;
}

/** Tunes PI for `values` with the Automatic method and validates the result; gives what Tune and Validate show. Each result
 * must be a new one, not the result of an earlier request. */
export async function tuneAndValidate(s: Session, studio: string, values: Record<string, string>): Promise<Record<string, string>> {
  await enterRequirements(s, studio, values);
  const before = await cdp.evalWebview(s.wb, studio, visible("[data-studio-tuning-result]"));
  await press(s, studio, "[data-engineer-tune]");
  const tuned = await cdp.pollValue("the tuning", () => cdp.evalWebview(s.wb, studio, visible("[data-studio-tuning-result]")),
    (text) => Boolean(text) && text !== before, 300_000);
  await tab(s, studio, "verify");
  const table = await cdp.evalWebview(s.wb, studio, visible("[data-studio-validation-result]"));
  await press(s, studio, "[data-engineer-verify]");
  const validated = await cdp.pollValue("the validation", () => cdp.evalWebview(s.wb, studio, visible("[data-studio-validation-result]")),
    (text) => Boolean(text) && text !== table, 600_000);
  return {
    tuned,
    tuneMessage: await cdp.evalWebview(s.wb, studio, message("tune")),
    validated,
    verifyMessage: await cdp.evalWebview(s.wb, studio, message("verify")),
  };
}
