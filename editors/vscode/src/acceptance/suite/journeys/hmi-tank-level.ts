// The hmi_tank_level walkthrough (specification 39, §11 and §12; specification 40): the tank's runtime runs on this computer,
// started from the command line as the README says. VS Code connects to it from the Target picker, the HMI Builder opens the
// Overview page and Live mode operates it: log in as `operator`, raise the setpoint to 2.6 m, see the level rise past 2.5 m and
// the T-101 level high alarm come in unacknowledged, acknowledge it, and lower the setpoint to 2.0 m until the alarm, back below
// 2.45 m, leaves the list (specification 40, §3.1). The runtime is built with the toolchain an earlier session's Start
// installed (specification 22). Each step records what it shows; the README's walkthrough quotes these values. A step that does
// not show its result fails the journey.
import * as cdp from "../cdp";
import { AlarmRow, alarmRows, clickOn, connectProjectRuntime, enter, goLive, logIn, numberIn, objectText, openBuilder, until } from "../hmi";
import { assertNoFailedStep, expect, open, Session, step } from "../session";

/** The level as the Overview's Level value shows it, in metres. */
const levelOf = (text: string) => numberIn(text, "m");

/** The setpoint as the Overview's Setpoint shows it (its value, before the limits under it), in metres. */
function setpointOf(text: string): number {
  const found = text.match(/^Setpoint\s*(-?\d+(?:\.\d+)?)\s*m/);
  return found ? Number(found[1]) : Number.NaN;
}

/** The T-101 level high alarm's row, not its high-high one. */
const highOf = (rows: AlarmRow[]) => rows.find((row) => /T-101 level high/.test(row.text) && !/high-high/.test(row.text));

suite("HMI tank level: operating the screens in Live mode", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "hmi-tank-level"); });

  test("connects, opens the Overview in Live mode, logs in, changes the setpoint and acknowledges the high alarm", async () => {
    let builder = "";
    const read = (script: string) => cdp.evalWebview(s.wb, builder, script);
    let started = 0;

    await step(s, "connected", () => connectProjectRuntime(s));
    await step(s, "live", async () => {
      builder = await openBuilder(s, "overview.toml");
      const shown = await goLive(s, builder, "setpoint-1");
      return { status: shown.status, session: shown.session, level: await read(objectText("value-1")), setpoint: await read(objectText("setpoint-1")) };
    });
    await step(s, "logged-in", async () => ({ session: await logIn(s, builder, "operator", "operator") }));
    await step(s, "setpoint", async () => {
      await enter(s, builder, "setpoint-1", "2.6");
      started = Date.now();
      const setpoint = await until(() => read(objectText("setpoint-1")), (text) => setpointOf(text) === 2.6, 15_000);
      expect(setpointOf(setpoint) === 2.6, `the Setpoint shows 2.60 m: ${setpoint}`);
      return { setpoint, level: await read(objectText("value-1")) };
    });
    await step(s, "high-alarm", async () => {
      const trend: Array<{ seconds: number; level: number; setpoint: string }> = [];
      let rows: AlarmRow[] = [];
      while (Date.now() - started < 120_000) {
        trend.push({ seconds: Math.round((Date.now() - started) / 100) / 10, level: levelOf(await read(objectText("value-1"))), setpoint: await read(objectText("setpoint-1")) });
        rows = JSON.parse(await read(alarmRows));
        if (highOf(rows)) break;
        await cdp.sleep(1_000);
      }
      const high = highOf(rows);
      expect(high, `T-101 level high comes in: ${JSON.stringify(rows)}`);
      expect(high?.acknowledge, `it waits for its acknowledgement: ${JSON.stringify(high)}`);
      return { trend, rows };
    });
    await step(s, "acknowledged", async () => {
      const high = highOf(JSON.parse(await read(alarmRows)));
      expect(high, "the high alarm is in the list");
      const pressed = await read(clickOn(`[data-alarm-action="acknowledge"][data-alarm="${high?.alarm ?? ""}"]`));
      expect(pressed === "CLICKED", `the row offers Acknowledge: ${pressed}`);
      const after = await until(async () => JSON.parse(await read(alarmRows)) as AlarmRow[],
        (now) => now.some((row) => row.alarm === high?.alarm && !row.acknowledge), 15_000);
      const row = after.find((candidate) => candidate.alarm === high?.alarm);
      expect(row && !row.acknowledge, `the alarm is acknowledged and stays while the level is high: ${JSON.stringify(after)}`);
      return { rows: after, level: await read(objectText("value-1")) };
    });
    await step(s, "cleared", async () => {
      await enter(s, builder, "setpoint-1", "2.0");
      const lowered = Date.now();
      const fall: Array<{ seconds: number; level: number }> = [];
      let rows: AlarmRow[] = [];
      // The alarm returns to normal below its limit less its deadband, 2.5 m - 0.05 m = 2.45 m; acknowledged and normal, it
      // leaves the list of active alarms.
      while (Date.now() - lowered < 120_000) {
        fall.push({ seconds: Math.round((Date.now() - lowered) / 100) / 10, level: levelOf(await read(objectText("value-1"))) });
        rows = JSON.parse(await read(alarmRows));
        if (!rows.some((row) => row.alarm === "Tank.Vessel.High")) break;
        await cdp.sleep(1_000);
      }
      expect(!rows.some((row) => row.alarm === "Tank.Vessel.High"), `the high alarm leaves the list once the level is back: ${JSON.stringify(rows)}`);
      return { fall, rows, setpoint: await read(objectText("setpoint-1")) };
    });
    assertNoFailedStep(s);
  });
});
