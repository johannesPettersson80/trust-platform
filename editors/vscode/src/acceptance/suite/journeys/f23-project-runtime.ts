// The workspace project's runtime runs on this computer, started from the command line as its README says (`trust-runtime
// build`, `trust-runtime run`), and VS Code opens the project folder it runs in. The side panel's Target picker offers it as
// "This project's runtime" while it answers (specification 25, section 5.1); choosing it and pressing Connect connects to it
// with the endpoint and token that trust-runtime reports for the project, the side panel's Target and the status bar name it,
// and when the runtime stops its status says it is not running. Choosing the Simulator puts every surface back on the
// Simulator. E2E_RUNTIME_PID is the runtime's process, which the journey stops as the user would stop it outside VS Code.
import * as vscode from "vscode";

import * as cdp from "../cdp";
import { assertNoFailedStep, describe as describeScreen, expect, open, openScreen, pressHome, Session, step } from "../session";

const PROJECT = "This project's runtime";

type Panel = { panel: string; status: string[] };

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

/** Opens the side panel's Target picker and gives its question. */
async function openPicker(s: Session): Promise<{ opened: string; question: cdp.QuickInput | null }> {
  await vscode.commands.executeCommand("workbench.view.extension.trust");
  await cdp.sleep(1_000);
  const opened = await cdp.evalWebview(s.wb, s.home ?? "",
    "var b=d.getElementById('targetButton');if(!b)return 'NONE';b.click();return 'CLICKED';");
  await cdp.sleep(1_500);
  return { opened, question: await cdp.quickInput(s.wb) };
}

async function panel(s: Session): Promise<Panel> {
  return { panel: (await describeScreen(s, s.home ?? "")).body.slice(0, 400), status: await cdp.statusBar(s.wb) };
}

const offers = (question: cdp.QuickInput | null, label: string) => (question?.items ?? []).some((item) => item.includes(label));
const names = (status: string[], label: string) => (status || []).some((item) => item.includes(label));

suite("F23: the project's own runtime in the Target picker", () => {
  let s: Session;
  suiteSetup(async () => { s = await open(process.env.E2E_SESSION || "f23-project-runtime"); });

  test("offers, selects and connects to the runtime started outside VS Code", async () => {
    await step(s, "offered", async () => {
      const picker = await openPicker(s);
      expect(picker.opened === "CLICKED", `the side panel has its Target button: ${picker.opened}`);
      expect(offers(picker.question, PROJECT), `the picker offers ${PROJECT}: ${JSON.stringify(picker.question)}`);
      return { ...(await panel(s)), ...picker };
    });
    await step(s, "selected", async () => {
      const picked = await cdp.answerQuickInput(s.wb, { pattern: `^\\s*${PROJECT}` });
      expect(picked !== "NO_ITEM", `the picker lets the user choose ${PROJECT}`);
      const shown = await until(() => panel(s), (p) => p.panel.includes(PROJECT), 10_000);
      expect(shown.panel.includes(PROJECT), `the side panel's Target names ${PROJECT}: ${shown.panel}`);
      return { picked, ...shown };
    });
    await step(s, "connected", async () => {
      const clicked = await pressHome(s, "^Connect$");
      // The tank example keeps debugging disabled (`debug_enabled = false`), so Connect reaches the runtime and the status bar
      // says so instead of attaching (specification 25, section 4.4); a runtime that allows debugging attaches and the side
      // panel offers Disconnect.
      const answered = (p: Panel) => names(p.status, `${PROJECT} · debugging disabled`) || /Disconnect/.test(p.panel);
      const shown = await until(() => panel(s), answered, 20_000);
      const notes = await cdp.notifications(s.wb).catch(() => [] as string[]);
      expect(!notes.some((note) => /Could not connect/.test(note)), `Connect reached the runtime: ${JSON.stringify(notes)}`);
      expect(answered(shown), `the status bar or the side panel names the runtime Connect reached: ${JSON.stringify(shown.status)} ${shown.panel}`);
      return { clicked, ...shown };
    });
    await step(s, "live-values", async () => {
      // Connect may have opened Live Values to say why there is no session; a closed one opens anew.
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      const live = await openScreen(s, "Live Values", () => pressHome(s, "Live Values"));
      await cdp.sleep(5_000);
      const text = (await describeScreen(s, live)).body.slice(0, 900);
      expect(!/Target\s+Simulator/.test(text), `Live Values is on the project's runtime, not the Simulator: ${text}`);
      return { text };
    });
    await step(s, "canvas", async () => {
      const canvas = await openScreen(s, "Devices & Connections", () => pressHome(s, "Devices & Connections"));
      await cdp.sleep(3_000);
      return { outcome: "info", text: (await describeScreen(s, canvas)).body.slice(0, 900) };
    });
    await step(s, "runtime-stopped", async () => {
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
      const pid = Number(process.env.E2E_RUNTIME_PID);
      let stopped = "NO_PID";
      if (pid > 0) {
        try {
          process.kill(pid, "SIGTERM");
          stopped = "SIGTERM";
        } catch (error) {
          stopped = (error as Error).message;
        }
      }
      expect(stopped === "SIGTERM", `the journey stops the runtime: ${stopped}`);
      const shown = await until(() => panel(s), (p) => /not running/i.test(p.panel), 30_000);
      // The picker opens before the checks, so that the next step can choose the Simulator whatever they find.
      const picker = await openPicker(s);
      expect(/not running/i.test(shown.panel), `the side panel says the project's runtime is not running: ${shown.panel}`);
      expect(offers(picker.question, PROJECT), `the selected entry stays in the picker: ${JSON.stringify(picker.question)}`);
      return { stopped, ...shown, ...picker };
    });
    await step(s, "simulator-selected", async () => {
      const picked = await cdp.answerQuickInput(s.wb, { pattern: "^\\s*Simulator" });
      expect(picked !== "NO_ITEM", "the picker lets the user choose the Simulator");
      const shown = await until(() => panel(s), (p) => names(p.status, "Simulator"), 10_000);
      expect(names(shown.status, "Simulator"), `the status bar is back on the Simulator: ${JSON.stringify(shown.status)}`);
      return { picked, ...shown };
    });
    await step(s, "picker-closed", async () => {
      await cdp.pressKey(s.wb, "Escape");
      await cdp.sleep(500);
      return panel(s);
    });
    assertNoFailedStep(s);
  });
});
