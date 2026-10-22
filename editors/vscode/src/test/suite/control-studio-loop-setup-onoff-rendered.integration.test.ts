import * as assert from "assert";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { attachControlStudio, attachWebview } from "./control-studio-rendered-harness";
import { captureRenderedWorkbenchCdp, connectCdp, devToolsPort, evaluateInWebview, httpJson, pollValue, requiredEnvironment } from "./plant-rendered-harness";

/** The README's Tune requirements for the cold room: a -7 °C test change from 12 °C, a 2 °C error limit and a 0.5 °C band that settles in 90 s. */
const COLD_ROOM_REQUIREMENTS: Record<string, number> = { referenceChange: -7, maximumError: 2, settlingBand: 0.5, settlingTimeSeconds: 90,
  // The validation duration is the journey's own choice: the README names none, and it must exceed the 90 s settling time.
  durationSeconds: 180 };

suite("Control Studio ON/OFF setup rendered", () => {
  test("sets up the program's own switching logic, opens it in Control Studio and tunes its switching band and minimum times", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    // The setup, then the supplied switching model and Tune, each wait on the engine.
    this.timeout(180000);
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
        for(const [name,value] of [['structure','onoff'],['name','Cold room'],['action','reverse'],['onoff.instance','P1'],['onoff.processVariable','P1.RoomC'],
          ['onoff.processVariable.unit','°C'],['onoff.processVariable.minimum','-10'],['onoff.processVariable.maximum','20'],['onoff.setpoint','P1.SetpointC'],
          ['onoff.manipulatedVariable','P1.CompressorPct'],['onoff.manipulatedVariable.unit','%'],['onoff.manipulatedVariable.minimum','0'],
          ['onoff.manipulatedVariable.maximum','100'],['onoff.mode','P1.AutoMode'],['onoff.manualOutput','P1.ManualPct'],['onoff.hysteresis','P1.HysteresisC'],
          ['onoff.minimumOn','P1.MinimumOnS'],['onoff.minimumOff','P1.MinimumOffS'],['onoff.equipmentMinimumOn','20'],['onoff.equipmentMinimumOff','20'],
          ['onoff.latch','P1.CoolingOn'],['onoff.dwellTimer','P1.DwellS']]) set(name,value);return 'filled';`);
      const state = JSON.parse(await inner(`return JSON.stringify({save:d.querySelector('[data-setup-save]').disabled,message:d.querySelector('[data-setup-message]').textContent,
        startup:d.querySelector('[data-setup-startup]').hidden,visible:[...d.querySelectorAll('[data-setup-section]')].filter(s=>!s.hidden).map(s=>s.dataset.setupSection)});`));
      assert.deepStrictEqual([state.save, state.startup, state.visible], [false, true, ["onoff"]],
        `the program writes only its own switching state, so no startup flag is needed: ${state.message}`);
      await shot("completed");
      await inner("d.querySelector('[data-setup-save]').click();return 'save';");
      const studio = await attachControlStudio(connection, port, "", "loop:cold-room");
      await pollValue("Control Studio ready", () => evaluateInWebview(connection, studio, "return d.querySelector('[data-studio-three-page]')?.dataset.hostReady;"), value => value === "true", 10000);
      await shot("studio");
      const saved = JSON.parse(fs.readFileSync(path.join(project, "control-loops", "cold-room.json"), "utf8"));
      // The engine validates the saved loop when Control Studio opens it (the open answer, awaited above), so no TypeScript check repeats
      // the validation (the deleted validateControlLoopDefinition).
      assert.deepStrictEqual([saved.controller.onOff.hysteresisSymbol, saved.controller.onOff.lowOutput, saved.controller.onOff.highOutput], ["P1.HysteresisC", 0, 100]);
      // The README's measured model is supplied as a plant model file (Identify, Supplied plant model); Tune then chooses the band and the
      // minimum times from the README's requirements, and the Tune result shows the ON/OFF section (specification 35, C3).
      const page = (body: string) => evaluateInWebview(connection, studio, body);
      const model = { kind: "transfer-function", revision: "cold-room-identified-switching", source: "provided", validation: "validated",
        evidence: ["Identified in Control Studio: the compressor switched from 0 to 100 % for 300 s"], delaySeconds: 1.1,
        numerator: [-0.0986], denominator: [57.5, 1],
        inputs: [{ id: "u", unit: saved.signals.manipulatedVariable.unit, operatingRange: saved.signals.manipulatedVariable.range }],
        outputs: [{ id: "y", unit: saved.signals.processVariable.unit, operatingRange: saved.signals.processVariable.range }],
        operatingPoint: { inputs: [0], outputs: [12] } };
      await page("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='supplied-model';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'supplied';");
      await page(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(JSON.stringify(model))}],'cold-room-model.json',{type:'application/json'}));const file=d.querySelector('[data-studio-model-file]');file.files=transfer.files;file.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'model imported';`);
      await pollValue("the supplied cold-room model", () => page("return d.querySelector('[data-studio-model-equation]')?.textContent??'';"), value => value.length > 0, 10000);
      await page("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-requirements]').click();return 'requirements';");
      await pollValue("the requirements editor", () => page("return String(!!d.querySelector('dialog[open]'));"), value => value === "true", 5000);
      await page(`const set=(key,value)=>{const field=d.querySelector('[data-studio-requirement='+key+']');field.value=String(value);field.dispatchEvent(new d.defaultView.Event('input',{bubbles:true}));};for(const [key,value] of ${JSON.stringify(Object.entries(COLD_ROOM_REQUIREMENTS))})set(key,value);d.querySelector('[data-studio-editor-done]').click();return 'requirements set';`);
      await page("d.querySelector('[data-engineer-tune]').click();return 'tune';");
      // A refused Tune leaves the page's sentence in its message, which the timeout reports with the wait.
      await pollValue("the ON/OFF section of the Tune result", () => page("return JSON.stringify({section:!!d.querySelector('[data-studio-on-off-design]'),message:d.querySelector('[data-studio-message=tune]')?.textContent??'',detail:d.querySelector('[data-studio-failure-detail=tune]')?.textContent??''});"),
        value => JSON.parse(value).section === true, 60000);
      const parameter = (symbol: string) => `d.querySelector('[data-studio-on-off-design] [data-studio-parameter="${symbol}"]')?.textContent??''`;
      const design = JSON.parse(await page(`return JSON.stringify({title:d.querySelector('[data-studio-tuning-result] p')?.textContent??'',hysteresis:${parameter(saved.controller.onOff.hysteresisSymbol)},`
        + `minimumOn:${parameter(saved.controller.onOff.minimumOnTimeSymbol)},minimumOff:${parameter(saved.controller.onOff.minimumOffTimeSymbol)},`
        + "levels:d.querySelector('[data-studio-on-off-levels]')?.textContent??''});")) as { title: string; hysteresis: string; minimumOn: string; minimumOff: string; levels: string };
      // README: the hysteresis is the half band, which equals the 0.5 °C settling band, and each minimum time is the compressor's own 20 s limit, which Tune never goes below.
      assert.strictEqual(design.title, "ON/OFF controller", JSON.stringify(design));
      assert.strictEqual(parseFloat(design.hysteresis), 0.5, "The hysteresis equals the 0.5 °C settling band (the half band equals the settling band)");
      assert.strictEqual(parseFloat(design.minimumOn), 20, "The minimum ON time is the compressor's own 20 s");
      assert.strictEqual(parseFloat(design.minimumOff), 20, "The minimum OFF time is the compressor's own 20 s");
      assert.match(design.levels, /OFF at 0\b.*ON at 100\b/, "The output levels are the declared 0 % and 100 %");
      await shot("tuned");
    } finally {
      connection.close();
      await vscode.commands.executeCommand("workbench.action.closeAllEditors");
    }
  });
});
