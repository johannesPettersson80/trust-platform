// The hmi_tank_level walkthrough (specification 39, §11 and §12; specification 40): the tank's runtime runs on this computer,
// started from the command line as the README says. VS Code connects to it from the Target picker, the HMI Builder opens the
// Overview page and Live mode operates it: log in as `operator`, raise the setpoint to 2.6 m, see the level rise past 2.5 m and
// the T-101 level high alarm come in unacknowledged, acknowledge it, and lower the setpoint to 2.0 m until the alarm, back below
// 2.45 m, leaves the list (specification 40, §3.1). The runtime is built with the toolchain an earlier session's Start
// installed (specification 22). Each step records what it shows; the README's walkthrough quotes these values. A step that does
// not show its result fails the journey.
import * as path from "path";
import * as vscode from "vscode";

import * as cdp from "../cdp";
import { assertNoFailedStep, expect, open, pressHome, Session, step } from "../session";

const PROJECT = "This project's runtime";

/** Reads `read` until `holds` or `ms` pass, and gives the last value. */
async function until<T>(read: () => Promise<T>, holds: (value: T) => boolean, ms: number): Promise<T> {
  const deadline = Date.now() + ms;
  let value = await read();
  while (!holds(value) && Date.now() < deadline) {
    await cdp.sleep(500);
    value = await read();
  }
  return value;
}

/** What the builder shows: its live status, whether it operates, its session bar, its buttons, and its objects. */
const builderState = String.raw`var q=function(s){return d.querySelector(s);};var t=function(e){return e?(e.innerText||e.textContent||'').replace(/\s+/g,' ').trim():null;};return JSON.stringify({status:t(q('[data-part="live-status"]')),state:(q('[data-part="live-status"]')||{}).dataset?q('[data-part="live-status"]').dataset.state:null,operating:(q('[data-operating]')||{dataset:{}}).dataset.operating||null,session:t(q('[data-part="session"]')),buttons:[...d.querySelectorAll('button')].filter(function(b){return b.offsetParent!==null;}).map(function(b){return (b.innerText||'').trim()+(b.dataset.action?' ['+b.dataset.action+']':'');}).filter(Boolean).slice(0,40),objects:[...d.querySelectorAll('[data-object]')].map(function(o){return o.getAttribute('data-object');}).slice(0,40),parts:[...new Set([...d.querySelectorAll('[data-part]')].map(function(p){return p.getAttribute('data-part');}))].slice(0,60),frames:d.querySelectorAll('iframe').length});`;

/** The text of a drawn object of the page, by its id. */
const objectText = (id: string) =>
  `var o=d.querySelector('[data-object="${id}"]');return o?(o.textContent||'').replace(/\s+/g,' ').trim():'';`;

/** The alarm rows as the alarm list draws them: each alarm's path, its text, and whether it offers Acknowledge. */
const alarmRows = String.raw`return JSON.stringify([...d.querySelectorAll('[data-part="alarm-row"]')].map(function(r){return {alarm:r.getAttribute('data-alarm'),text:(r.textContent||'').replace(/\s+/g,' ').trim(),acknowledge:Boolean(r.querySelector('[data-alarm-action="acknowledge"]'))};}));`;

type AlarmRow = { alarm: string; text: string; acknowledge: boolean };

/** The level as the Overview's Level value shows it, in metres. */
function levelOf(text: string): number {
  const found = text.match(/(-?\d+(?:\.\d+)?)\s*m\b/);
  return found ? Number(found[1]) : Number.NaN;
}

/** The setpoint as the Overview's Setpoint shows it (its value, before the limits under it), in metres. */
function setpointOf(text: string): number {
  const found = text.match(/^Setpoint\s*(-?\d+(?:\.\d+)?)\s*m/);
  return found ? Number(found[1]) : Number.NaN;
}

/** Clicks the first element matching `selector` as the user does (the display listens for bubbling clicks). */
const clickOn = (selector: string) =>
  `var e=d.querySelector(${JSON.stringify(selector)});if(!e)return 'NONE';e.dispatchEvent(new MouseEvent('click',{bubbles:true}));return 'CLICKED';`;

/** Fills the open dialog's fields by name and submits it, as typing and pressing its submit button do. */
const fillAndSubmit = (form: string, fields: Record<string, string>) =>
  `var f=d.querySelector(${JSON.stringify(form)});if(!f)return 'NO_DIALOG';var w=f.ownerDocument.defaultView;var set=Object.getOwnPropertyDescriptor(w.HTMLInputElement.prototype,'value').set;var values=${JSON.stringify(fields)};for(var k in values){var i=f.querySelector('input[name="'+k+'"]');if(!i)return 'NO_FIELD '+k;set.call(i,values[k]);i.dispatchEvent(new w.Event('input',{bubbles:true}));}var b=f.querySelector('button[type="submit"]');if(!b)return 'NO_SUBMIT';b.click();return 'SUBMITTED';`;

suite("HMI tank level: operating the screens in Live mode", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "hmi-tank-level"); });

  test("connects, opens the Overview in Live mode, logs in, changes the setpoint and acknowledges the high alarm", async () => {
    let builder = "";
    const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";

    await step(s, "connected", async () => {
      await vscode.commands.executeCommand("workbench.view.extension.trust");
      await cdp.sleep(1_000);
      const opened = await cdp.evalWebview(s.wb, s.home ?? "",
        "var b=d.getElementById('targetButton');if(!b)return 'NONE';b.click();return 'CLICKED';");
      expect(opened === "CLICKED", `the side panel has its Target button: ${opened}`);
      await cdp.sleep(1_500);
      const picked = await cdp.answerQuickInput(s.wb, { pattern: `^\\s*${PROJECT}` });
      expect(picked !== "NO_ITEM", `the picker offers ${PROJECT}`);
      await cdp.sleep(1_500);
      const clicked = await pressHome(s, "^Connect$");
      // The tank example keeps debugging disabled, so Connect reaches the runtime without attaching (specification 25, 4.4).
      const connected = (items: string[]) => items.some((item) => item.includes(`${PROJECT} · debugging disabled`));
      const status = await until(() => cdp.statusBar(s.wb), connected, 20_000);
      expect(connected(status), `the status bar says Connect reached ${PROJECT}: ${JSON.stringify(status)}`);
      return { picked, clicked, status };
    });
    await step(s, "builder", async () => {
      await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(path.join(folder, "hmi", "pages", "overview.toml")));
      // Connect may have opened Live Values too; the builder is the webview with the Live button.
      builder = await cdp.attachWebview(s.wb, "HMI Builder", "d.querySelector('[data-action=\"live\"]')", 60_000);
      await cdp.sleep(3_000);
      const state = JSON.parse(await cdp.evalWebview(s.wb, builder, builderState)) as { operating: string | null; buttons: string[] };
      expect(state.buttons.some((label) => label.endsWith("[live]")), `the builder offers Live: ${JSON.stringify(state.buttons.slice(0, 8))}`);
      return { operating: state.operating, buttons: state.buttons.slice(0, 8) };
    });
    await step(s, "live", async () => {
      const pressed = await cdp.evalWebview(s.wb, builder, "var b=d.querySelector('[data-action=\"live\"]');if(!b)return 'NONE';b.click();return 'CLICKED';");
      expect(pressed === "CLICKED", `the builder offers Live: ${pressed}`);
      const shown = await until(async () => JSON.parse(await cdp.evalWebview(s.wb, builder, builderState)),
        (state: { status: string | null; operating: string | null; session: string | null; objects: string[] }) =>
          Boolean(state.session) && state.objects.includes("setpoint-1"), 30_000);
      expect(shown.session !== null, `Live mode opens the operating view with its session bar: ${JSON.stringify(shown)}`);
      expect(shown.objects.includes("setpoint-1"), `the Overview's objects are drawn: ${JSON.stringify(shown.objects)}`);
      return {
        pressed, status: shown.status, operating: shown.operating, session: shown.session,
        level: await cdp.evalWebview(s.wb, builder, objectText("value-1")),
        setpoint: await cdp.evalWebview(s.wb, builder, objectText("setpoint-1")),
      };
    });
    await step(s, "logged-in", async () => {
      const asked = await cdp.evalWebview(s.wb, builder, clickOn('[data-part="session"] [data-action="login"]'));
      expect(asked === "CLICKED", `the session bar offers Log in: ${asked}`);
      await cdp.sleep(500);
      const sent = await cdp.evalWebview(s.wb, builder, fillAndSubmit("form", { user: "operator", password: "operator" }));
      expect(sent === "SUBMITTED", `the login dialog takes the user and password: ${sent}`);
      const session = await until(() => cdp.evalWebview(s.wb, builder,
        "var b=d.querySelector('[data-part=\"session\"]');return b?(b.textContent||'').replace(/\\s+/g,' ').trim():'';"),
      (text) => /operator/i.test(text) && /Log out/.test(text), 15_000);
      expect(/operator/i.test(session) && /Log out/.test(session), `the session bar names the operator: ${session}`);
      return { session };
    });
    const trend: Array<{ seconds: number; level: number; setpoint: string }> = [];
    let started = 0;
    await step(s, "setpoint", async () => {
      const opened = await cdp.evalWebview(s.wb, builder, clickOn('[data-object="setpoint-1"]'));
      expect(opened === "CLICKED", `the Setpoint is drawn: ${opened}`);
      await cdp.sleep(500);
      const sent = await cdp.evalWebview(s.wb, builder, fillAndSubmit("form", { value: "2.6" }));
      expect(sent === "SUBMITTED", `the setpoint entry takes 2.6: ${sent}`);
      started = Date.now();
      const setpoint = await until(() => cdp.evalWebview(s.wb, builder, objectText("setpoint-1")), (text) => setpointOf(text) === 2.6, 15_000);
      expect(setpointOf(setpoint) === 2.6, `the Setpoint shows 2.60 m: ${setpoint}`);
      return { setpoint, level: await cdp.evalWebview(s.wb, builder, objectText("value-1")) };
    });
    await step(s, "high-alarm", async () => {
      let rows: AlarmRow[] = [];
      while (Date.now() - started < 120_000) {
        const level = levelOf(await cdp.evalWebview(s.wb, builder, objectText("value-1")));
        trend.push({ seconds: Math.round((Date.now() - started) / 100) / 10, level, setpoint: await cdp.evalWebview(s.wb, builder, objectText("setpoint-1")) });
        rows = JSON.parse(await cdp.evalWebview(s.wb, builder, alarmRows));
        if (rows.some((row) => /T-101 level high/.test(row.text) && !/high-high/.test(row.text))) break;
        await cdp.sleep(1_000);
      }
      const high = rows.find((row) => /T-101 level high/.test(row.text) && !/high-high/.test(row.text));
      expect(high, `T-101 level high comes in: ${JSON.stringify(rows)}`);
      expect(high?.acknowledge, `it waits for its acknowledgement: ${JSON.stringify(high)}`);
      return { trend, rows };
    });
    await step(s, "acknowledged", async () => {
      const rows: AlarmRow[] = JSON.parse(await cdp.evalWebview(s.wb, builder, alarmRows));
      const high = rows.find((row) => /T-101 level high/.test(row.text) && !/high-high/.test(row.text));
      expect(high, "the high alarm is in the list");
      const pressed = await cdp.evalWebview(s.wb, builder,
        clickOn(`[data-alarm-action="acknowledge"][data-alarm="${high?.alarm ?? ""}"]`));
      expect(pressed === "CLICKED", `the row offers Acknowledge: ${pressed}`);
      const after = await until(async () => JSON.parse(await cdp.evalWebview(s.wb, builder, alarmRows)) as AlarmRow[],
        (now) => now.some((row) => row.alarm === high?.alarm && !row.acknowledge), 15_000);
      const row = after.find((candidate) => candidate.alarm === high?.alarm);
      expect(row && !row.acknowledge, `the alarm is acknowledged and stays while the level is high: ${JSON.stringify(after)}`);
      return { rows: after, level: await cdp.evalWebview(s.wb, builder, objectText("value-1")) };
    });
    await step(s, "cleared", async () => {
      const opened = await cdp.evalWebview(s.wb, builder, clickOn('[data-object="setpoint-1"]'));
      expect(opened === "CLICKED", `the Setpoint is drawn: ${opened}`);
      await cdp.sleep(500);
      const sent = await cdp.evalWebview(s.wb, builder, fillAndSubmit("form", { value: "2.0" }));
      expect(sent === "SUBMITTED", `the setpoint entry takes 2.0: ${sent}`);
      const lowered = Date.now();
      const fall: Array<{ seconds: number; level: number }> = [];
      let rows: AlarmRow[] = [];
      // The alarm returns to normal below its limit less its deadband, 2.5 m - 0.05 m = 2.45 m; acknowledged and normal, it
      // leaves the list of active alarms.
      while (Date.now() - lowered < 120_000) {
        fall.push({ seconds: Math.round((Date.now() - lowered) / 100) / 10, level: levelOf(await cdp.evalWebview(s.wb, builder, objectText("value-1"))) });
        rows = JSON.parse(await cdp.evalWebview(s.wb, builder, alarmRows));
        if (!rows.some((row) => row.alarm === "Tank.Vessel.High")) break;
        await cdp.sleep(1_000);
      }
      expect(!rows.some((row) => row.alarm === "Tank.Vessel.High"), `the high alarm leaves the list once the level is back: ${JSON.stringify(rows)}`);
      return { fall, rows, setpoint: await cdp.evalWebview(s.wb, builder, objectText("setpoint-1")) };
    });
    assertNoFailedStep(s);
  });
});
