import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

export async function exerciseVisiblePlotToolbar(inner: EvaluateWebview,
  capture: (name: string) => Promise<void>, prefix: string, frozen: boolean): Promise<void> {
  const click = (action: string) => inner(`const b=d.querySelector('[data-chart-action=${action}]');const r=b.getBoundingClientRect();
    if(b.disabled||r.width===0||r.height===0)throw Error('Toolbar action unavailable: ${action}');b.click();return d.querySelector('[data-chart-status]')?.textContent??'clicked';`);
  await click("inspect");
  assert.strictEqual(await inner("return String(d.activeElement?.getAttribute('role')==='img');"), "true", "Inspect must focus the plotted evidence");
  await capture(`${prefix}-inspect`);
  await inner("d.defaultView.__toolbarBefore=[...d.querySelectorAll('.plot-view:not([hidden]) canvas')].map(c=>c.toDataURL());return 'retain image';");
  const zoomInStatus = await click("zoom-in");
  if (frozen) await pollValue("model zoom redraw", () => inner("return String([...d.querySelectorAll('.plot-view:not([hidden]) canvas')].some((c,i)=>c.toDataURL()!==d.defaultView.__toolbarBefore[i]));"), value => value === "true", 5000);
  else assert.match(zoomInStatus, /Zoomed in/);
  await capture(`${prefix}-zoom-in`);
  const zoomOutStatus = await click("zoom-out");
  if (!frozen) assert.match(zoomOutStatus, /Zoomed out/);
  await capture(`${prefix}-zoom-out`);
  const resetStatus = await click("reset");
  if (!frozen) assert.match(resetStatus, /View reset/);
  await capture(`${prefix}-fit`);
  await click("expand");
  assert.strictEqual(await inner("return d.querySelector('[data-control-studio]').dataset.plotExpanded;"), "true");
  await capture(`${prefix}-expanded`);
  await click("expand");
  assert.strictEqual(await inner("return d.querySelector('[data-control-studio]').dataset.plotExpanded;"), "false");
  await capture(`${prefix}-restored`);
}
