import * as assert from "assert";
import { recordingArchive } from "./control-studio-recording-archive";
import { withStudio } from "./control-studio-three-page.integration.test";
import { pollValue } from "./plant-rendered-harness";

suite("Control Studio periodic analysis", () => {
  test("imports a retained recording, calculates host-owned FRF, and reopens it", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(90000);
    let loop: any;
    await withStudio(async (inner, reopen, capture) => {
      const recordingValue = JSON.parse(recordingArchive(loop, "periodic-import", 1));
      // The input is a sine about the middle of the loop's output range, so it stays strictly inside the limits: the engine refuses a
      // record on a limit (measured_frequency.rs). The output is twice the input's deviation plus 5, so the measured magnitude is 2.
      // The engine reads the archived run, so the same samples go into it; `rawSamples` gets them too.
      const { outputMinimum, outputMaximum } = loop.limits, middle = (outputMinimum + outputMaximum) / 2, amplitude = 5;
      const sampleAt = (index: number) => {
        const input = middle + amplitude * Math.sin(index * 0.5);
        return { input, output: 2 * (input - middle) + 5 };
      };
      for (const [index, sample] of recordingValue.recording.rawSamples.entries()) {
        const { input, output } = sampleAt(index);
        sample.appliedMv = input; sample.requestedMv = input; sample.pv = output;
      }
      const archived = recordingValue.engineerStudy.campaigns[0].runs[0];
      const inputSymbol = loop.signals.manipulatedVariable.symbol, outputSymbol = loop.signals.processVariable.symbol;
      for (const [index, sample] of archived.run.samples.entries()) {
        const { input, output } = sampleAt(index);
        sample.values[inputSymbol].value = input; sample.values[outputSymbol].value = output;
      }
      // The archive's own data-bounds requirement on the output (-20 to 20, as `recordingArchive` declares it) still holds for this
      // response, which stays between -5 and 15, so its verdict stands; the evaluation names the run it judged, which is this one.
      archived.evaluation.run = structuredClone(archived.run);
      const recording = JSON.stringify(recordingValue);
      // The file is opened with the default method, so the engine keeps it as a recording of the study (`importRecording`, spec 35
      // §0.1a) and the measured response reads it there; a file opened with Recorded-data fit stays with that manual identification.
      // The engine keeps the file as a recording of the study: a one-run file answers `recording-needed`, a checked file `checked`.
      await inner("d.defaultView.__periodicRecordingImported=false;d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.identification-recording'&&['checked','recording-needed'].includes(event.data.payload?.status)&&typeof event.data.payload?.recordingId==='string'){d.defaultView.__periodicRecordingImported=true;d.defaultView.__periodicRecordingId=event.data.payload.recordingId;}});return 'recording listener';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(recording)}],'periodic.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'recording';`);
      await pollFor(inner, "recording import", "return String(d.defaultView.__periodicRecordingImported===true);", 15000);
      // The engine's analysis messages (`control-studio.analysis.*`) reach the page; the measured response is an operation.
      // The engine answers the tools that apply to the recording, so the option is awaited before it is chosen.
      await pollValue("measured response in the Analyze list", () => inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze]').click();return String(!!d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"measuredFrequency\"]'));"), value => value === "true", 15000);
      await inner("d.defaultView.__periodicEvents=[];d.defaultView.addEventListener('message',event=>{if(typeof event.data?.kind==='string'&&event.data.kind.startsWith('control-studio.analysis.'))d.defaultView.__periodicEvents.push(event.data);});d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"measuredFrequency\"]').click();return 'periodic';");
      await inner("d.querySelector('[data-analysis-periodic-frequencies]').value='5';d.querySelector('[data-analysis-periodic-segments]').value='5';d.querySelector('[data-analysis-periodic-coherence]').value='0.8';d.querySelector('[data-analysis-periodic-startup]').value='16';d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      // The update that finishes the operation carries the envelope; its answer is `result.measuredFrequency` (B9.9, B10.2). It names
      // the recording it measured by the study's identifier, the one the import answered.
      const periodic = JSON.parse(await pollValue("host periodic result", () => inner("return JSON.stringify(d.defaultView.__periodicEvents.filter(e=>e.kind==='control-studio.analysis.update'&&e.state==='finished'&&e.envelope?.result?.measuredFrequency?.recordingId===d.defaultView.__periodicRecordingId).map(e=>e.envelope.result.measuredFrequency));"), value => JSON.parse(value).length > 0, 20000))[0];
      // An isolated simulation's run names no loop revision, so the kept recording takes the revision of the loop it is imported into
      // (archive.rs, `archived_recording`): the revision of the loop the journey opened.
      assert.strictEqual(periodic.recordingRevision, loop.revision, "the kept recording carries the revision of the loop it was imported into");
      assert.ok(Math.abs(periodic.points[0].magnitude - 2) < 0.1, JSON.stringify(periodic.points[0]));
      assert.ok(periodic.points[0].coherence > 0.8, JSON.stringify(periodic.points[0]));
      // The Recording field names the recording in the engine's words, what it recorded and when (B10.3), not by the study's `rec-…`
      // digest (wrong behaviour caught: the digest was the only name the engineer saw).
      const recordingField = await pollValue("recording words", () => inner("return d.querySelector('[data-analysis-periodic-recording]')?.value??'';"), value => value.length > 0, 10000);
      assert.ok(!recordingField.startsWith("rec-") && / UTC$/.test(recordingField) && recordingField === periodic.recordingLabel, recordingField);
      await capture("periodic-import");
      assert.ok(Number(await inner("let count=0;for(const canvas of d.querySelectorAll('[data-three-page-analysis-plot] canvas')){if(!canvas.width||!canvas.height)continue;const ctx=canvas.getContext('2d');if(!ctx)continue;const start=Math.floor(canvas.height*.15);const pixels=ctx.getImageData(0,start,canvas.width,canvas.height-start).data;for(let i=0;i<pixels.length;i+=4)if(pixels[i+2]>180&&pixels[i+2]>pixels[i]+50&&pixels[i+2]>pixels[i+1]+25)count++;}return String(count);")) > 6, "A single measured FRF point must be visible in the plotted data area, beyond its legend");
      await reopen();
      // Reopening restores the measured response and calculates it again; the option is awaited before the list chooses it.
      await pollValue("measured response after reopening", () => inner("d.querySelector('[data-engineer-stage-tab=tune]').click();const list=d.querySelector('[data-studio-analyze-list]');if(!list.hidden)d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze]').click();return String(!!d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"measuredFrequency\"]'));"), value => value === "true", 15000);
      await inner("d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"measuredFrequency\"]').click();return 'periodic reopened';");
      await pollFor(inner, "reopened periodic result", "return String(d.querySelector('[data-three-page-analysis-summary]')?.textContent??'').includes('Measured');", 20000);
    }, { loop: value => { loop = value; } });
  });
});

async function pollFor(inner: (body: string) => Promise<string>, label: string, body: string, timeoutMs: number): Promise<void> {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (await inner(body) === "true") return;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  assert.fail(`${label} did not become available`);
}
