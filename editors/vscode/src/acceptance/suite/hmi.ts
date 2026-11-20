// The steps that the HMI journeys share (specification 39, §11 and §12): connecting to the project's runtime from the Target
// picker, opening a page in the HMI Builder's Live mode, logging in, and reading and using the runtime display's drawn objects,
// popups and alarm list as the user does.
import * as path from "path";
import * as vscode from "vscode";

import * as cdp from "./cdp";
import { expect, pressHome, Session } from "./session";

const PROJECT = "This project's runtime";

/** Reads `read` until `holds` or `ms` pass, and gives the last value. */
export async function until<T>(read: () => Promise<T>, holds: (value: T) => boolean, ms: number): Promise<T> {
  const deadline = Date.now() + ms;
  let value = await read();
  while (!holds(value) && Date.now() < deadline) {
    await cdp.sleep(500);
    value = await read();
  }
  return value;
}

/** Chooses the project's runtime in the side panel's Target picker and presses Connect. The HMI examples keep debugging
 * disabled, so Connect reaches the runtime without attaching (specification 25, 4.4). */
export async function connectProjectRuntime(s: Session): Promise<{ picked: string; clicked: string; status: string[] }> {
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
  const connected = (items: string[]) => items.some((item) => item.includes(`${PROJECT} · debugging disabled`));
  const status = await until(() => cdp.statusBar(s.wb), connected, 20_000);
  expect(connected(status), `the status bar says Connect reached ${PROJECT}: ${JSON.stringify(status)}`);
  return { picked, clicked, status };
}

/** What the builder shows: its live status, whether it operates, its session bar, its visible buttons and its objects. */
const builderState = String.raw`var q=function(s){return d.querySelector(s);};var t=function(e){return e?(e.innerText||e.textContent||'').replace(/\s+/g,' ').trim():null;};var st=q('[data-part="live-status"]');return JSON.stringify({status:t(st),state:st?st.dataset.state:null,operating:(q('[data-operating]')||{dataset:{}}).dataset.operating||null,session:t(q('[data-part="session"]')),buttons:[...d.querySelectorAll('button')].filter(function(b){return b.offsetParent!==null;}).map(function(b){return (b.innerText||'').trim()+(b.dataset.action?' ['+b.dataset.action+']':'');}).filter(Boolean).slice(0,12),objects:[...d.querySelectorAll('[data-object]')].map(function(o){return o.getAttribute('data-object');})});`;

export type BuilderState = { status: string | null; state: string | null; operating: string | null; session: string | null; buttons: string[]; objects: string[] };

/** Opens `page` of the project's HMI in the HMI Builder; gives the builder's webview. Connect may have opened Live Values
 * too, so the builder is the webview with the Live button. */
export async function openBuilder(s: Session, page: string): Promise<string> {
  const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? "";
  await vscode.commands.executeCommand("vscode.open", vscode.Uri.file(path.join(folder, "hmi", "pages", page)));
  const builder = await cdp.attachWebview(s.wb, "HMI Builder", "d.querySelector('[data-action=\"live\"]')", 60_000);
  await cdp.sleep(2_000);
  return builder;
}

/** Presses Live and waits until the operating view draws the object `marker` with its session bar; gives what it shows. */
export async function goLive(s: Session, builder: string, marker: string): Promise<BuilderState> {
  const pressed = await cdp.evalWebview(s.wb, builder, "var b=d.querySelector('[data-action=\"live\"]');if(!b)return 'NONE';b.click();return 'CLICKED';");
  expect(pressed === "CLICKED", `the builder offers Live: ${pressed}`);
  const shown = await until(async () => JSON.parse(await cdp.evalWebview(s.wb, builder, builderState)) as BuilderState,
    (state) => Boolean(state.session) && state.objects.includes(marker), 30_000);
  expect(shown.operating === "true" && shown.session !== null, `Live mode opens the operating view with its session bar: ${JSON.stringify(shown)}`);
  expect(shown.objects.includes(marker), `the page's objects are drawn: ${JSON.stringify(shown.objects.slice(0, 20))}`);
  return shown;
}

/** The text of the session bar. */
const sessionText = "var b=d.querySelector('[data-part=\"session\"]');return b?(b.textContent||'').replace(/\\s+/g,' ').trim():'';";

/** Logs in from the session bar as `user` with `password`; gives the session bar's text then. */
export async function logIn(s: Session, builder: string, user: string, password: string): Promise<string> {
  const asked = await cdp.evalWebview(s.wb, builder, clickOn('[data-part="session"] [data-action="login"]'));
  expect(asked === "CLICKED", `the session bar offers Log in: ${asked}`);
  await cdp.sleep(500);
  const sent = await cdp.evalWebview(s.wb, builder, fillAndSubmit("form", { user, password }));
  expect(sent === "SUBMITTED", `the login dialog takes the user and password: ${sent}`);
  const session = await until(() => cdp.evalWebview(s.wb, builder, sessionText),
    (text) => text.includes(user) && /Log out/.test(text), 15_000);
  expect(session.includes(user) && /Log out/.test(session), `the session bar names ${user}: ${session}`);
  return session;
}

/** A script that gives the text of the drawn object `id`, inside `scope` (a popup) or anywhere on the page. */
export const objectText = (id: string, scope = "") =>
  `var o=d.querySelector(${JSON.stringify(`${scope ? `${scope} ` : ""}[data-object="${id}"]`)});return o?(o.textContent||'').replace(/\\s+/g,' ').trim():'';`;

/** A script that says whether the drawn object `id` inside `scope` is held (greyed with a reason) or locked to a role. */
export const objectHold = (id: string, scope = "") =>
  `var o=d.querySelector(${JSON.stringify(`${scope ? `${scope} ` : ""}[data-object="${id}"]`)});if(!o)return 'NONE';var g=o.closest('[data-held],[data-locked]')||o.querySelector('[data-held],[data-locked]');return JSON.stringify({held:g?g.getAttribute('data-held'):null,locked:g?g.getAttribute('data-locked'):null});`;

/** The first number with the unit `unit` in `text`. */
export function numberIn(text: string, unit: string): number {
  const found = text.match(new RegExp(`(-?\\d+(?:\\.\\d+)?)\\s*${unit.replace(/[.*+?^${}()|[\]\\%]/g, "\\$&")}`));
  return found ? Number(found[1]) : Number.NaN;
}

/** The alarm rows as the alarm list draws them: each alarm's path, its text, and whether it offers Acknowledge. */
export const alarmRows = String.raw`return JSON.stringify([...d.querySelectorAll('[data-part="alarm-row"]')].map(function(r){return {alarm:r.getAttribute('data-alarm'),text:(r.textContent||'').replace(/\s+/g,' ').trim(),acknowledge:Boolean(r.querySelector('[data-alarm-action="acknowledge"]'))};}));`;

export type AlarmRow = { alarm: string; text: string; acknowledge: boolean };

/** A script that clicks the first element matching `selector` as the user does (the display listens for bubbling clicks). */
export const clickOn = (selector: string) =>
  `var e=d.querySelector(${JSON.stringify(selector)});if(!e)return 'NONE';e.dispatchEvent(new MouseEvent('click',{bubbles:true}));return 'CLICKED';`;

/** A script that fills the open dialog's fields by name and submits it, as typing and pressing its submit button do. */
export const fillAndSubmit = (form: string, fields: Record<string, string>) =>
  `var f=d.querySelector(${JSON.stringify(form)});if(!f)return 'NO_DIALOG';var w=f.ownerDocument.defaultView;var set=Object.getOwnPropertyDescriptor(w.HTMLInputElement.prototype,'value').set;var values=${JSON.stringify(fields)};for(var k in values){var i=f.querySelector('input[name="'+k+'"]');if(!i)return 'NO_FIELD '+k;set.call(i,values[k]);i.dispatchEvent(new w.Event('input',{bubbles:true}));}var b=f.querySelector('button[type="submit"]');if(!b)return 'NO_SUBMIT';b.click();return 'SUBMITTED';`;

/** Enters `value` in the entry of the drawn object `id` (inside `scope`, a popup, or on the page) and sends it. */
export async function enter(s: Session, builder: string, id: string, value: string, scope = ""): Promise<void> {
  const opened = await cdp.evalWebview(s.wb, builder, clickOn(`${scope ? `${scope} ` : ""}[data-object="${id}"]`));
  expect(opened === "CLICKED", `${id} is drawn: ${opened}`);
  await cdp.sleep(500);
  const sent = await cdp.evalWebview(s.wb, builder, fillAndSubmit("form", { value }));
  expect(sent === "SUBMITTED", `the entry of ${id} takes ${value}: ${sent}`);
}
