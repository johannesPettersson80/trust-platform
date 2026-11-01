// The control-pid-commissioning walkthrough (specification 35, B3 to B7): on the unmodified example, Start, then Control
// Studio's Identify plant (Automatic), Tune (PI, Automatic, the README's requirements: a 1 °C test setpoint change, at most
// 0.1 °C steady-state error, settled within 12 s in a ±0.05 °C band, a 24 s validation), Validate controller, Apply (the
// response read from Live Values each second for 40 s) and Restore previous settings. Every step records what it shows; the
// README's walkthrough quotes these results. A step that does not reach its result fails the journey.
import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, Session, step } from "../session";
import { enterRequirements, identify, message, openStudio, press, requirements, startSimulator, tab, visible } from "../studio";

// The setpoint, the temperature and the output as Live Values shows them: each row's name, its address, then its value.
const liveSignals = String.raw`var t=d.body.innerText;function v(n){var m=t.match(new RegExp(n.replace(/\./g,'\\.')+'\\s+%[A-Z]+[0-9.]+\\s+([-+0-9.eE]+)'));return m?Number(m[1]):null;}return JSON.stringify({setpoint:v('P1.Setpoint'),temperature:v('P1.ProcessValue'),output:v('P1.ManipulatedValue')});`;
// The visible buttons of Control Studio with their state and data attributes.
const controls = "return JSON.stringify({" +
  "buttons:[...d.querySelectorAll('button')].filter(function(b){return b.offsetParent!==null&&b.innerText.trim();}).map(function(b){return b.innerText.trim()+(b.disabled?' (disabled)':'')+' |'+[...b.attributes].map(function(a){return a.name;}).filter(function(n){return n.indexOf('data-')===0;}).join(',');})});";

suite("Commissioning the temperature loop in Control Studio", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "pid-commissioning"); });

  test("identifies, tunes, validates, applies and restores", async () => {
    let studio = "";
    let live = "";
    await step(s, "running", () => startSimulator(s));
    await step(s, "identified", async () => {
      studio = await openStudio(s);
      return identify(s, studio);
    });
    await step(s, "requirements", async () => ({
      set: await enterRequirements(s, studio, requirements("12 s", "24 s")),
      message: await cdp.evalWebview(s.wb, studio, message("tune")),
    }));
    await step(s, "tuned", async () => {
      await press(s, studio, "[data-engineer-tune]");
      const result = await cdp.pollValue("the tuning", () => cdp.evalWebview(s.wb, studio, visible("[data-studio-tuning-result]")), Boolean, 300_000);
      return { result, message: await cdp.evalWebview(s.wb, studio, message("tune")) };
    });
    await step(s, "validated", async () => {
      await tab(s, studio, "verify");
      await press(s, studio, "[data-engineer-verify]");
      const table = await cdp.pollValue("the validation", () => cdp.evalWebview(s.wb, studio, visible("[data-studio-validation-result]")), Boolean, 600_000);
      return { table, message: await cdp.evalWebview(s.wb, studio, message("verify")), controls: JSON.parse(await cdp.evalWebview(s.wb, studio, controls)) };
    });
    // A physical target asks for a review and its confirmation before Apply and Restore write anything; the Simulator does
    // not (B6.1). The confirmation is pressed when it appears.
    const confirmIfAsked = async (action: string) => {
      const asked = await cdp.pollValue(`the ${action} review`, () => cdp.evalWebview(s.wb, studio, visible(`[data-engine-confirm="${action}"]`)),
        Boolean, 8_000).catch(() => "");
      if (asked) await press(s, studio, `[data-engine-confirm="${action}"]`);
      return asked;
    };
    await step(s, "applied", async () => {
      expect(/^Apply$/.test(await cdp.evalWebview(s.wb, studio, visible("[data-engineer-apply]"))), "the Validate page offers Apply");
      await press(s, studio, "[data-engineer-apply]");
      const review = await confirmIfAsked("apply");
      // Live Values opened with Start; its webview is the one with the force policy banner.
      live = await cdp.attachWebview(s.wb, "Live Values", "d.body&&/Force policy/.test(d.body.innerText)", 60_000);
      const trend: Record<string, unknown>[] = [];
      const started = Date.now();
      while (Date.now() - started < 40_000) {
        trend.push({ seconds: Math.round((Date.now() - started) / 100) / 10, ...JSON.parse(await cdp.evalWebview(s.wb, live, liveSignals)) });
        await cdp.sleep(1_000);
      }
      const answered = await cdp.pollValue("the Apply result", () => cdp.evalWebview(s.wb, studio, message("verify")),
        (text) => /passed|applied|failed|refused/i.test(text || ""), 300_000).catch((error: Error) => error.message);
      expect(/passed/i.test(answered || ""), `Apply passes its commissioning check: ${answered}`);
      return {
        review, trend, answered,
        application: await cdp.evalWebview(s.wb, studio, visible("[data-studio-application]")),
        controls: JSON.parse(await cdp.evalWebview(s.wb, studio, controls)),
      };
    });
    await step(s, "restored", async () => {
      const offered = await cdp.pollValue("Restore previous settings", () => cdp.evalWebview(s.wb, studio, visible("[data-engineer-apply]")),
        (text) => /Restore previous settings/.test(text || ""), 120_000).catch(() => "");
      const button = offered ? "[data-engineer-apply]" : "[data-engineer-restore]";
      const restore = await cdp.evalWebview(s.wb, studio,
        "var b=[...d.querySelectorAll('button')].find(function(x){return x.offsetParent!==null&&/Restore previous settings/.test(x.innerText);});" +
        "if(!b)return 'NONE';if(b.disabled)return 'DISABLED';b.click();return 'CLICKED';");
      expect(restore === "CLICKED", `Restore previous settings is offered: ${restore} (${button})`);
      const review = await confirmIfAsked("restore");
      const answered = await cdp.pollValue("the Restore result", () => cdp.evalWebview(s.wb, studio, message("verify")),
        (text) => /restor/i.test(text || ""), 120_000).catch((error: Error) => error.message);
      await cdp.sleep(5_000);
      return { review, answered, live: JSON.parse(await cdp.evalWebview(s.wb, live, liveSignals)),
        controls: JSON.parse(await cdp.evalWebview(s.wb, studio, controls)) };
    });
    assertNoFailedStep(s);
  });
});
