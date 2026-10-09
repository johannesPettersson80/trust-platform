import * as assert from "assert";
import { recordingArchive } from "./control-studio-recording-flow.test";
import { withStudio } from "./control-studio-three-page.integration.test";

suite("Control Studio periodic analysis", () => {
  test("imports a retained recording, calculates host-owned FRF, and reopens it", async function () {
    if (process.env.TRUST_CONTROL_STUDIO_THREE_PAGE !== "1") this.skip();
    this.timeout(90000);
    let loop: any;
    await withStudio(async (inner, reopen, capture) => {
      const recordingValue = JSON.parse(recordingArchive(loop, "periodic-import", 1));
      for (const [index, sample] of recordingValue.recording.rawSamples.entries()) {
        const value = index < 16 ? 0 : Math.sin(index * 0.1 * 5);
        sample.appliedMv = value; sample.requestedMv = value; sample.pv = 2 * value;
      }
      const recording = JSON.stringify(recordingValue);
      await inner("const method=d.querySelector('[data-engineer-acquisition-method]');method.value='recorded-fit';method.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'recorded-fit';");
      await inner("d.defaultView.__periodicRecordingImported=false;d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.identification-recording'&&event.data.payload?.status==='imported')d.defaultView.__periodicRecordingImported=true;});return 'recording listener';");
      await inner(`const transfer=new d.defaultView.DataTransfer();transfer.items.add(new d.defaultView.File([${JSON.stringify(recording)}],'periodic.json',{type:'application/json'}));const input=d.querySelector('[data-identify-import-file]');input.files=transfer.files;input.dispatchEvent(new d.defaultView.Event('change',{bubbles:true}));return 'recording';`);
      await pollFor(inner, "recording import", "return String(d.defaultView.__periodicRecordingImported===true);", 15000);
      await inner("d.defaultView.__periodicEvents=[];d.defaultView.addEventListener('message',event=>{if(event.data?.kind==='control-studio.engineering.result'&&event.data.stage==='general-model')d.defaultView.__periodicEvents.push(event.data.payload);});d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"periodic-frequency\"]').click();return 'periodic';");
      assert.strictEqual(await inner("return String(!!d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"periodic-frequency\"]'));"), "true");
      await inner("d.querySelector('[data-analysis-periodic-frequencies]').value='5';d.querySelector('[data-analysis-periodic-segments]').value='5';d.querySelector('[data-analysis-periodic-coherence]').value='0.8';d.querySelector('[data-analysis-periodic-startup]').value='16';d.querySelector('[data-engineer-tune]').click();return 'calculate';");
      await pollFor(inner, "host periodic result", "return String(d.defaultView.__periodicEvents.some(payload=>payload.status==='periodic-complete'&&payload.periodic?.recordingId==='periodic-import'));", 20000);
      const periodic = JSON.parse(await inner("const event=d.defaultView.__periodicEvents.find(payload=>payload.status==='periodic-complete');return JSON.stringify(event.periodic);"));
      assert.strictEqual(periodic.recordingRevision, "1");
      assert.ok(Math.abs(periodic.result.points[0].magnitude - 2) < 0.1, JSON.stringify(periodic.result.points[0]));
      assert.ok(periodic.result.points[0].coherence > 0.8, JSON.stringify(periodic.result.points[0]));
      await capture("periodic-import");
      assert.ok(Number(await inner("let count=0;for(const canvas of d.querySelectorAll('[data-three-page-analysis-plot] canvas')){if(!canvas.width||!canvas.height)continue;const ctx=canvas.getContext('2d');if(!ctx)continue;const start=Math.floor(canvas.height*.15);const pixels=ctx.getImageData(0,start,canvas.width,canvas.height-start).data;for(let i=0;i<pixels.length;i+=4)if(pixels[i+2]>180&&pixels[i+2]>pixels[i]+50&&pixels[i+2]>pixels[i+1]+25)count++;}return String(count);")) > 6, "A single measured FRF point must be visible in the plotted data area, beyond its legend");
      await reopen();
      await inner("d.querySelector('[data-engineer-stage-tab=tune]').click();d.querySelector('[data-studio-analyze]').click();d.querySelector('[data-studio-analyze-list] [data-studio-analysis-tool=\"periodic-frequency\"]').click();return 'periodic reopened';");
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
