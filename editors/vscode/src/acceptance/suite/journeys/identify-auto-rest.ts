// Identify plant with a Manual method on a loop found in Auto reads the loop until it has settled before it switches it to
// Manual, shows that wait ("Waiting for the loop to settle in Auto"), and then steps the output from where the controller
// holds it (specification 35, B3.4 step 5). The runner's copy of control-pid-commissioning starts in Auto with its setpoint
// at 30 °C, so after Start the controller still drives the heater when Identify plant is pressed. Every step checks what the
// specification says it shows, and the journey fails when a step does.
import * as cdp from "../cdp";
import { assertNoFailedStep, click, expect, open, Session, setValue, step } from "../session";
import { openStudio, startSimulator, visible } from "../studio";

const modelText = visible("[data-studio-model-result]");
const messageText = visible('[data-studio-message="model"]');

suite("Identify on a loop in Auto waits for it to settle", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "identify-auto-rest"); });

  test("waits for the loop to settle, then identifies from where the controller holds it", async () => {
    let studio = "";
    await step(s, "running", () => startSimulator(s));
    await step(s, "studio", async () => {
      studio = await openStudio(s);
      const options: { value: string; text: string }[] = JSON.parse(await cdp.evalWebview(s.wb, studio,
        "var el=d.querySelector('[data-engineer-acquisition-method]');if(!el)return '[]';" +
        "return JSON.stringify([...el.options].map(function(o){return {value:o.value,text:o.text};}));"));
      const stepResponse = options.find((option) => /^Step response/i.test(option.text));
      expect(stepResponse, `the Method list offers Step response: ${JSON.stringify(options)}`);
      const chosen = await setValue(s, studio, "[data-engineer-acquisition-method]", stepResponse!.value);
      expect(chosen === stepResponse!.value, `Step response is chosen: ${chosen}`);
      return { options, chosen };
    });
    await step(s, "rest-wait", async () => {
      const clicked = await click(s, studio, "^Identify plant$");
      const shown = await cdp.pollValue("the rest wait", () => cdp.evalWebview(s.wb, studio, messageText),
        (text) => /Waiting for the loop to settle in Auto/.test(text || ""), 60_000);
      const model = await cdp.evalWebview(s.wb, studio, modelText);
      expect(!model, `no model before the wait ends: ${model}`);
      return { clicked, shown };
    });
    await step(s, "identified", async () => {
      const model = await cdp.pollValue("the identified model", () => cdp.evalWebview(s.wb, studio, modelText), Boolean, 300_000);
      const shown = await cdp.evalWebview(s.wb, studio, messageText);
      expect(!/Waiting for the loop to settle/.test(shown || ""), `the wait has ended: ${shown}`);
      return { model, message: shown };
    });
    assertNoFailedStep(s);
  });
});
