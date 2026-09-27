import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";
import { pollValue } from "./plant-rendered-harness";

type Capture = (name: string) => Promise<void>;

/** Exercises the plot tabs that are available after a real controller candidate exists. */
export async function exerciseEngineeringPlotViews(
  inner: EvaluateWebview,
  capture: Capture,
): Promise<void> {
  const clickMode = async (mode: string): Promise<void> => {
    assert.strictEqual(await inner(`const b=[...d.querySelectorAll('[data-control-studio-mode]')].find(e=>e.textContent.trim()===${JSON.stringify(mode)});
      const r=b?.getBoundingClientRect();if(!b||b.disabled||!r?.width||!r.height)return 'unavailable';b.click();return 'clicked';`), "clicked", `${mode} mode must be reachable`);
    await pollValue(`${mode} plot tabs`, () => inner("return d.querySelector('[data-control-plot-tabs]')?.textContent??'';"), value => value.length > 0, 5000);
  };

  const clickPlot = async (plot: string, stage: string): Promise<void> => {
    const state = JSON.parse(await inner(`const b=d.querySelector('[data-plot-tab=${plot}]');
      const r=b?.getBoundingClientRect();return JSON.stringify({exists:Boolean(b),disabled:Boolean(b?.disabled),reason:b?.title||'',width:r?.width||0,height:r?.height||0});`));
    assert.strictEqual(state.exists, true, `${plot} must have a discoverable plot tab in ${stage}`);
    if (state.disabled) {
      assert.ok(state.reason, `${plot} unavailable state must explain its prerequisite`);
      await capture(`engineering-${plot}-unavailable`);
      return;
    }
    assert.ok(state.width > 0 && state.height > 0, `${plot} tab must be visibly reachable`);
    await inner(`d.querySelector('[data-plot-tab=${plot}]').click();return 'selected';`);
    await pollValue(`${plot} selected`, () => inner(`const b=d.querySelector('[data-plot-tab=${plot}]');const v=d.querySelector('[data-plot-view=${plot}]');return JSON.stringify({selected:b?.getAttribute('aria-selected'),hidden:v?.hidden});`), value => {
      const result = JSON.parse(value); return result.selected === "true" && result.hidden === false;
    }, 5000);
    const geometry = JSON.parse(await pollValue(`${plot} painted canvas`, () => inner(`const v=d.querySelector('[data-plot-view=${plot}]');const host=v?.querySelector('.analysis-plot');const canvas=host?.querySelector('canvas');const r=host?.getBoundingClientRect();const c=canvas?.getBoundingClientRect();return JSON.stringify({width:r?.width||0,height:r?.height||0,canvasWidth:c?.width||0,canvasHeight:c?.height||0});`), value => { const g=JSON.parse(value); return g.width>120&&g.height>80&&g.canvasWidth>80&&g.canvasHeight>50; }, 5000));
    assert.ok(geometry.width > 120 && geometry.height > 80, `${plot} chart host must be visible with meaningful dimensions`);
    assert.ok(geometry.canvasWidth > 80 && geometry.canvasHeight > 50, `${plot} must paint a rendered chart canvas`);
    await capture(`engineering-${plot}`);
  };

  await clickMode("Tune");
  for (const plot of ["root-locus", "nyquist", "nichols", "sensitivity"]) {
    await clickPlot(plot, "Tune");
  }

  await clickMode("Validate");
  await clickPlot("requirements", "Validate");
  await clickMode("Tune");
  await inner("d.querySelector('[data-plot-tab=response]').click();return 'response';");
  await pollValue("returned prediction view", () => inner("return String(!d.querySelector('[data-plot-view=engineering-response]').hidden&&d.querySelector('[data-engineering-time] canvas').width>0);"), value => value === "true", 5000);
}
