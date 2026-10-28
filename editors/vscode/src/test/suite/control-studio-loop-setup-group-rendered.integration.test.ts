import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

suite("Control Studio PID group setup rendered", () => {
  test("sets up two interacting Control.PID instances as one group and opens it in Control Studio", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(90000);
    const project = vscode.workspace.workspaceFolders![0].uri.fsPath;
    await vscode.extensions.getExtension("trust-platform.trust-lsp")?.activate();
    const port = await devToolsPort(requiredEnvironment("TRUST_PLANT_RENDERED_USER_DATA_DIR"), 15000);
    const connection = await connectCdp((await httpJson(port, "/json/version")).webSocketDebuggerUrl);
    const shot = (name: string) => captureRenderedWorkbenchCdp(connection, requiredEnvironment("TRUST_CONTROL_STUDIO_RENDERED_SCREENSHOT").replace(/\.png$/, `-${name}.png`));
    try {
      void vscode.commands.executeCommand("trust-lsp.controlStudio.configure");
      const form = await attachWebview(connection, port, "[data-loop-setup][data-ready=\"true\"]");
      const inner = (body: string) => evaluateInWebview(connection, form, body);
      await inner(`const set=(name,value)=>{const e=d.querySelector('[data-setup-field="'+name+'"]');e.value=value;e.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));};
        const include=name=>{const e=d.querySelector('[data-setup-field="'+name+'"]');e.checked=true;e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));};
        set('structure','group');set('name','Zone temperatures');include('group.0.include');include('group.1.include');
        for(const n of [0,1]){set('group.'+n+'.processVariable.unit','°C');set('group.'+n+'.processVariable.minimum','20');set('group.'+n+'.processVariable.maximum','60');
          set('group.'+n+'.manipulatedVariable.unit','%');set('group.'+n+'.manipulatedVariable.minimum','0');set('group.'+n+'.manipulatedVariable.maximum','100');}
        set('group.mode','P1.ZonesAuto');return 'filled';`);
      assert.deepStrictEqual(JSON.parse(await inner(`return JSON.stringify(['group.1.parameterRecord','group.1.processVariable','group.1.manipulatedVariable','group.1.setpoint','group.1.manualOutput']
        .map(name=>d.querySelector('[data-setup-field="'+name+'"]').value));`)),
      ["P1.Zone2Parameters", "P1.Zone2.StateValue.ProcessValue", "P1.Heater2Pct", "P1.Zone2SetpointC", "P1.Zone2ManualPct"],
      "each member is filled in from its own compiled calls; its measurement is the PID's own");
      assert.strictEqual(await inner("return String(d.querySelector('[data-setup-startup]').hidden);"), "false",
        "the program initializes its PIDs at startup, so the group needs the startup flag");
      await inner(`const e=d.querySelector('[data-setup-field="startupFlag"]');e.value='P1.Initialized';e.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'flag';`);
      assert.strictEqual(await inner("return String(d.querySelector('[data-setup-save]').disabled);"), "false", await inner("return d.querySelector('[data-setup-message]').textContent;"));
      const layout = JSON.parse(await inner(`const probe=d.createElement('div');probe.style.borderTop='1px solid var(--trust-border-subtle)';d.body.append(probe);
        const expected=d.defaultView.getComputedStyle(probe).borderTopColor;probe.remove();const member=d.querySelector('[data-group-member]');const style=d.defaultView.getComputedStyle(member);
        return JSON.stringify({color:style.borderTopColor,expected,width:style.borderTopWidth,checkbox:d.querySelector('[data-setup-field="group.0.include"]').getBoundingClientRect().width});`));
      assert.deepStrictEqual([layout.color, layout.width], [layout.expected, "1px"], "member blocks use the shared subtle border, not the browser's fieldset frame");
      assert.ok(layout.checkbox < 32, `the include checkbox keeps its own size: ${layout.checkbox}px`);
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:zone-temperatures");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "zone-temperatures.json"), "utf8"));
      assert.deepStrictEqual(saved.controller.group.members.map((member: { instance: string }) => member.instance), ["P1.Zone1", "P1.Zone2"]);
      assert.deepStrictEqual(saved.controller.project.seed, { kind: "decentralized-pid", family: "PI" });
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
