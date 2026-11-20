// The hmi_equipment walkthrough (specification 39, §11 and §12; trust-plant-library §6.1 and §8): the example's runtime runs on
// this computer, started from the command line as its README says. VS Code connects to it, the HMI Builder opens the Overview
// in Live mode, and the operator uses the equipment panels: P-101 runs in Auto with Start and Stop held ("Auto: switch to
// Manual"); LIC-101 holds the level, follows a setpoint of 1.8 m, closes the valve in Manual at 0 % until LT-101's Low alarm
// comes in, and refills the tank in Auto without a bump. The runtime is built with the toolchain an earlier session's Start
// installed (specification 22). Each step records what it shows; the README quotes these values. A step that does not show
// its result fails the journey.
import * as cdp from "../cdp";
import {
  clickOn, connectProjectRuntime, enter, goLive, logIn, numberIn, objectHold, objectText, openBuilder, until,
} from "../hmi";
import { assertNoFailedStep, expect, open, Session, step } from "../session";

const POPUP = '[data-part="popup"]';

/** The level as the Overview's tank gauge shows it, in metres. */
const levelOf = (text: string) => numberIn(text, "m");

/** Closes every open popup, so that the next one is the only one (a step that failed may leave its popup open). */
const closePopups = "var n=0;[...d.querySelectorAll('[data-part=\"popup\"] [data-action=\"close\"]')].forEach(function(b){b.dispatchEvent(new MouseEvent('click',{bubbles:true}));n++;});return String(n);";

/** The pump panel's mode options, the active one marked with `*`. */
const modeOptions = `return JSON.stringify([...d.querySelectorAll('${POPUP} [data-object="mode"] [data-option]')].map(function(o){return o.getAttribute('data-option')+(o.getAttribute('data-active')?'*':'');}));`;

/** What the top bar says of the alarms: its count and its newest alarm. */
const alarmBar = "var c=d.querySelector('[data-part=\"alarm-count\"]'),n=d.querySelector('[data-part=\"alarm-newest\"]');var t=function(e){return e?(e.textContent||'').replace(/\\s+/g,' ').trim():'';};return JSON.stringify({count:t(c),newest:t(n)});";

suite("HMI equipment: operating the equipment panels in Live mode", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "hmi-equipment"); });

  test("runs the pump in Auto, moves the level with LIC-101, closes the valve in Manual and refills in Auto", async () => {
    let builder = "";
    const read = (script: string) => cdp.evalWebview(s.wb, builder, script);
    /** The loop panel's measurement in metres, read every second for at most `ms` until `done`; gives the readings. */
    const follow = async (done: (level: number) => boolean, ms: number) => {
      const readings: Array<{ seconds: number; level: number; output: string }> = [];
      const started = Date.now();
      while (Date.now() - started < ms) {
        const level = levelOf(await read(objectText("measurement", POPUP)));
        readings.push({ seconds: Math.round((Date.now() - started) / 100) / 10, level, output: await read(objectText("manual-output", POPUP)) });
        if (done(level)) break;
        await cdp.sleep(1_000);
      }
      return readings;
    };

    await step(s, "connected", () => connectProjectRuntime(s));
    await step(s, "live", async () => {
      builder = await openBuilder(s, "overview.toml");
      const shown = await goLive(s, builder, "p-101");
      return {
        status: shown.status,
        level: await read(objectText("level-gauge")),
        pump: await read(objectText("pump-state")),
        valve: await read(objectText("valve-reading")),
      };
    });
    await step(s, "logged-in", async () => ({ session: await logIn(s, builder, "operator", "operator") }));
    await step(s, "pump", async () => {
      await read(closePopups);
      const opened = await read(clickOn('[data-object="p-101"]'));
      expect(opened === "CLICKED", `P-101 is drawn: ${opened}`);
      try {
        // The panel draws at once and marks its live values as they come.
        const mode = JSON.parse(await until(() => read(modeOptions), (text) => text.includes("*"), 10_000)) as string[];
        const state = await read(objectText("state", POPUP));
        const start = JSON.parse(await read(objectHold("start", POPUP)));
        const stop = JSON.parse(await read(objectHold("stop", POPUP)));
        const reason = await read(objectText("reason", POPUP));
        expect(/Running/.test(state), `P-101 runs: ${state}`);
        expect(mode.includes("Auto*"), `P-101 is in Auto: ${JSON.stringify(mode)}`);
        expect(start.held === "true" && stop.held === "true", `in Auto Start and Stop are held (greyed): ${JSON.stringify({ start, stop })}`);
        expect(/Auto: switch to Manual/.test(reason), `the panel says why: ${reason}`);
        return { state, mode, start, stop, reason, starts: await read(objectText("starts", POPUP)) };
      } finally {
        await read(closePopups);
        await cdp.sleep(1_000);
      }
    });
    await step(s, "level-held", async () => {
      await read(closePopups);
      const opened = await read(clickOn('[data-object="lic-101"]'));
      expect(opened === "CLICKED", `LIC-101 is drawn: ${opened}`);
      await until(() => read(objectText("measurement", POPUP)), (text) => Number.isFinite(levelOf(text)), 10_000);
      const mode = await until(() => read(objectText("mode", POPUP)), (text) => /Auto|Manual/.test(text), 10_000);
      expect(/Auto/.test(mode), `LIC-101 is in Auto: ${mode}`);
      // A cold start begins with the valve closed (the loop starts from its output, 0): the level dips, then LIC-101 brings it
      // back to its setpoint of 1.5 m.
      const readings = await follow((level) => level >= 1.45, 150_000);
      expect(readings[readings.length - 1].level >= 1.45, `LIC-101 brings the level back to 1.5 m: ${JSON.stringify(readings.slice(-3))}`);
      return { mode, setpoint: await read(objectText("setpoint", POPUP)), readings };
    });
    await step(s, "setpoint", async () => {
      await enter(s, builder, "setpoint", "1.8", POPUP);
      const setpoint = await until(() => read(objectText("setpoint", POPUP)), (text) => numberIn(text, "m") === 1.8, 15_000);
      expect(numberIn(setpoint, "m") === 1.8, `the setpoint shows 1.80 m: ${setpoint}`);
      const readings = await follow((level) => level >= 1.78, 150_000);
      expect(readings[readings.length - 1].level >= 1.78, `the level follows to 1.8 m: ${JSON.stringify(readings.slice(-3))}`);
      return { setpoint, readings };
    });
    await step(s, "manual-closed", async () => {
      // The panel's Mode is a switch between Auto and Manual: a press writes the other one (plant-pid panel).
      const pressed = await read(clickOn(`${POPUP} [data-object="mode"]`));
      expect(pressed === "CLICKED", `the Mode switch is drawn: ${pressed}`);
      const mode = await until(() => read(objectText("mode", POPUP)), (text) => /Manual/.test(text), 15_000);
      expect(/Manual/.test(mode), `LIC-101 is in Manual: ${mode}`);
      await enter(s, builder, "manual-output", "0", POPUP);
      const alarms: Array<{ seconds: number; level: number; count: string; newest: string }> = [];
      const started = Date.now();
      while (Date.now() - started < 90_000) {
        const bar = JSON.parse(await read(alarmBar)) as { count: string; newest: string };
        const level = levelOf(await read(objectText("measurement", POPUP)));
        alarms.push({ seconds: Math.round((Date.now() - started) / 100) / 10, level, ...bar });
        if (/low/i.test(bar.newest) && level < 0.3) break;
        await cdp.sleep(1_000);
      }
      const last = alarms[alarms.length - 1];
      expect(last.level < 0.3 && /low/i.test(last.newest), `the level falls and LT-101's Low alarm comes in: ${JSON.stringify(alarms.slice(-3))}`);
      return { mode, output: await read(objectText("manual-output", POPUP)), alarms };
    });
    await step(s, "auto-refill", async () => {
      // The panel's Mode is a switch between Auto and Manual: a press writes the other one (plant-pid panel).
      const pressed = await read(clickOn(`${POPUP} [data-object="mode"]`));
      expect(pressed === "CLICKED", `the Mode switch is drawn: ${pressed}`);
      const mode = await until(() => read(objectText("mode", POPUP)), (text) => /Auto/.test(text), 15_000);
      expect(/Auto/.test(mode), `LIC-101 is in Auto again: ${mode}`);
      const readings = await follow((level) => level >= 1.0, 120_000);
      expect(readings[readings.length - 1].level >= 1.0, `LIC-101 refills the tank: ${JSON.stringify(readings.slice(-3))}`);
      return { mode, readings, alarms: JSON.parse(await read(alarmBar)) };
    });
    assertNoFailedStep(s);
  });
});
