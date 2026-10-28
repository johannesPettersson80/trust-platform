import * as assert from "assert";
import * as fs from "fs";

import {
  beginPlantDraft,
  bindFirstAddressedSignals,
  captureRenderedWorkbench,
  openRenderedPlantSession,
  pollSave,
  pollValue,
  sleep,
  type RenderedPlantSession,
} from "./plant-rendered-harness";

const RENDERED_TEST_ENABLED = process.env.TRUST_PLANT_RENDERED_TEST === "1";

type ModelRoute = {
  kind: string;
  name: string;
  explanation: string;
  dynamics: string[];
  starting: string[];
  ports: string[];
  editor?: "piecewise" | "state-space";
};

const MODEL_ROUTES: readonly ModelRoute[] = [
  {
    kind: "static_gain",
    name: "Static gain",
    explanation: "The output changes immediately in proportion to the input.",
    dynamics: ["gain", "input_bias", "output_bias"],
    starting: [],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "first_order",
    name: "First order",
    explanation: "The output moves gradually toward each new input.",
    dynamics: ["gain", "time_constant_s", "input_bias", "output_bias"],
    starting: ["Starting output"],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "first_order_dead_time",
    name: "First order with dead time",
    explanation: "The output starts changing after a delay, then settles gradually.",
    dynamics: ["gain", "time_constant_s", "dead_time_s"],
    starting: ["Starting value"],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "integrator",
    name: "Integrator",
    explanation: "The output keeps accumulating while the input differs from its resting point; it does not settle by itself.",
    dynamics: ["gain", "input_bias", "output_bias", "state_minimum", "state_maximum"],
    starting: ["Starting accumulated value"],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "second_order",
    name: "Second order",
    explanation: "The output can overshoot or oscillate before it settles.",
    dynamics: ["gain", "natural_frequency_rad_s", "damping_ratio", "input_bias", "output_bias"],
    starting: ["Starting displacement", "Starting rate of change"],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "piecewise_linear",
    name: "Piecewise linear",
    explanation: "A table of points defines an immediate nonlinear input-to-output curve.",
    dynamics: [],
    starting: [],
    ports: ["Command / PLC output", "Feedback / PLC input"],
    editor: "piecewise",
  },
  {
    kind: "state_space",
    name: "State space",
    explanation: "Coupled internal states turn one or more inputs into one or more outputs.",
    dynamics: [],
    starting: [],
    ports: ["Input 1", "Output 1"],
    editor: "state-space",
  },
  {
    kind: "gravity_tank",
    name: "Gravity-drained tank",
    explanation: "Inflow raises the level; gravity drains it faster as the level rises.",
    dynamics: ["area_m2", "outflow_coefficient_m2_sqrt_m_per_s"],
    starting: ["Starting level (m)"],
    ports: ["Command / PLC output", "Feedback / PLC input"],
  },
  {
    kind: "gravity_tank_cascade",
    name: "Cascaded gravity tanks",
    explanation: "The upstream tank freely drains into an equal downstream tank; coupled balances preserve the transferred water.",
    dynamics: ["area_m2", "outflow_coefficient_m2_sqrt_m_per_s"],
    starting: ["Starting upstream level (m)", "Starting downstream level (m)"],
    ports: ["Pump flow (m³/s)", "Upstream level (m)", "Downstream level (m)"],
  },
  {
    kind: "robot_joint",
    name: "Gravity-loaded robot joint",
    explanation: "Torque changes joint angle and speed; gravity depends on angle, and friction resists motion.",
    dynamics: ["inertia_kg_m2", "gravity_torque_nm", "viscous_friction_nm_s_per_rad", "coulomb_friction_nm", "friction_smoothing_rad_s"],
    starting: ["Starting angle (rad)", "Starting angular speed (rad/s)"],
    ports: ["Torque command (N·m)", "Joint angle (rad)", "Angular speed (rad/s)"],
  },
] as const;

if (RENDERED_TEST_ENABLED) {
  suite("Rendered plant editor", function () {
    this.timeout(120_000);

    test("CPUI-004 all model routes", async function () {
      const session = await openRenderedPlantSession();
      try {
        await beginPlantDraft(session);
        assert.deepStrictEqual(await readModelOptions(session), MODEL_ROUTES.map(({ kind, name }) => ({ value: kind, text: name })));

        const gates = JSON.parse(await session.inner("return JSON.stringify({equipment:d.querySelectorAll('[data-plant-equipment-chooser],[name=plant-equipment]').length,categories:d.querySelectorAll('[data-plant-model-category]').length,expert:d.querySelectorAll('[data-plant-expert-mode]').length});"));
        assert.deepStrictEqual(gates, { equipment: 0, categories: 0, expert: 0 }, "all supported models must use the direct model chooser without another gate");

        await session.setValue("select[name=plant-model]", "first_order_dead_time");
        await session.inner("var select=d.querySelector('select[name=plant-model]');select.focus();return select.value;");
        await session.pressKey("ArrowDown", "ArrowDown");
        await pollSelectedModel(session, "integrator", "keyboard model selection");

        for (const route of MODEL_ROUTES) {
          await selectModel(session, route.kind);
          await pollValue(`${route.name} schematic`, () => session.inner("return d.querySelector('[data-plant-model-diagram]')?.getAttribute('data-plant-model-diagram')||'';"), (value) => value === route.kind, 8_000);
          const surface = await settledModelSurface(session, route);
          assert.strictEqual(surface.diagram.kind, route.kind, `${route.name} should render its own Typical behavior schematic`);
          assert.strictEqual(surface.diagram.role, "img", `${route.name} schematic should expose an image role`);
          assert.ok(surface.diagram.label.includes(route.name), `${route.name} schematic should have a model-specific accessible name`);
          assert.strictEqual(surface.diagram.caption, "Typical behavior", `${route.name} schematic should be qualitative rather than a numerical prediction`);
          assert.deepStrictEqual(surface.diagram.textCollisions, [], `${route.name} schematic labels should not overlap: ${JSON.stringify(surface.diagram.textCollisions)}`);
          if (route.kind === "first_order" || route.kind === "first_order_dead_time" || route.kind === "second_order") {
            const shape = JSON.parse(await retried(() => session.inner("var curve=d.querySelector('[data-plant-model-diagram] svg').querySelectorAll('path')[2];var sample=(distance)=>{var p=curve.getPointAtLength(distance);return {x:p.x,y:p.y};};return JSON.stringify({start:sample(28),early:sample(29),later:sample(80),next:sample(81),delayed:sample(43)});")));
            assert.ok(Math.abs(shape.start.x - 58) < 0.1 && Math.abs(shape.start.y - 58) < 0.1,
              `${route.name} must share the input step's time origin, not imply an extra hidden delay`);
            if (route.kind === "first_order") {
              const earlySlope = (shape.start.y - shape.early.y) / (shape.early.x - shape.start.x);
              const lateSlope = (shape.later.y - shape.next.y) / (shape.next.x - shape.later.x);
              assert.ok(earlySlope > lateSlope && lateSlope >= 0,
                "first order must respond immediately then slow down, not draw an S-shaped response");
            } else if (route.kind === "first_order_dead_time") {
              assert.ok(Math.abs(shape.delayed.y - 58) < 0.1, "dead time must retain a visible flat response after the step");
            }
          }
          assert.strictEqual(surface.explanation, route.explanation, `${route.name} should explain its behavior in plain language`);
          assert.deepStrictEqual(surface.dynamics, route.dynamics, `${route.name} should expose only its owned response parameters`);
          assert.deepStrictEqual(surface.starting, route.starting, `${route.name} should expose its named starting state separately`);
          assert.deepStrictEqual(surface.ports, route.ports, `${route.name} should expose all required model ports`);
          assert.strictEqual(surface.piecewiseRows, route.editor === "piecewise" ? 3 : 0, `${route.name} should show the correct dedicated piecewise editor`);
          assert.deepStrictEqual(surface.stateSpace, route.editor === "state-space" ? { states: "1", inputs: "1", outputs: "1", matrices: ["a:0:0", "b:0:0", "c:0:0", "d:0:0"], initial: 1 } : undefined, `${route.name} should show the correct default state-space editor`);
          assert.strictEqual(surface.temperatureExample, route.kind === "first_order_dead_time", "the optional temperature proposal belongs only to FOPDT");
          assert.strictEqual(surface.manualOpen, 0, `${route.name} must not require manual signal entry to choose or inspect the model`);
          await captureModelEvidence(session, route);
        }

        await selectModel(session, "piecewise_linear");
        await bindFirstAddressedSignals(session);
        await pollSave(session.inner, false, "valid piecewise-linear route");
        await saveAndReopenPlant(session);
        const reopened = await settledModelSurface(session, MODEL_ROUTES[5]);
        assert.strictEqual(await selectedModel(session), "piecewise_linear", "saved Piecewise linear identity should reopen exactly");
        assert.strictEqual(reopened.explanation, MODEL_ROUTES[5].explanation);
        assert.strictEqual(reopened.diagram.kind, "piecewise_linear");
        assert.strictEqual(reopened.piecewiseRows, 3, "saved Piecewise linear points should reopen in the dedicated editor");
      } finally {
        session.close();
      }
    });
  });
}

async function selectModel(session: RenderedPlantSession, model: string): Promise<void> {
  await session.setValue("select[name=plant-model]", model);
  await pollSelectedModel(session, model, `${model} selection`);
}

async function pollSelectedModel(session: RenderedPlantSession, model: string, label: string): Promise<void> {
  await pollValue(label, () => selectedModel(session), (value) => value === model, 8_000);
}

async function selectedModel(session: RenderedPlantSession): Promise<string> {
  return session.inner("return d.querySelector('select[name=plant-model]')?.value||'';");
}

async function readModelOptions(session: RenderedPlantSession): Promise<Array<{ value: string; text: string }>> {
  return JSON.parse(await session.inner("var select=d.querySelector('select[name=plant-model]');return JSON.stringify([...select.options].map((option)=>({value:option.value,text:(option.textContent||'').trim()})));"));
}

/** `read`, read again while the diagram it reads is being drawn again. */
async function retried<T>(read: () => Promise<T>): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await read();
    } catch (error) {
      if (attempt >= 20) throw error;
      await sleep(100);
    }
  }
}

/**
 * The model's surface once the editor shows Rust's answer for the selected model: its sections can
 * be missing and its fields can still be the previous model's while that answer is on its way. Past
 * the deadline the last surface read is returned, so that the route's assertions name what differs.
 */
async function settledModelSurface(session: RenderedPlantSession, route: ModelRoute): Promise<any> {
  const deadline = Date.now() + 8_000;
  let last: any;
  while (Date.now() < deadline) {
    try {
      last = await readModelSurface(session);
      if (last.diagram.kind === route.kind &&
        JSON.stringify(last.dynamics) === JSON.stringify(route.dynamics) &&
        JSON.stringify(last.starting) === JSON.stringify(route.starting) &&
        JSON.stringify(last.ports) === JSON.stringify(route.ports)) {
        return last;
      }
    } catch {
      // A section is not shown yet.
    }
    await sleep(100);
  }
  return last ?? readModelSurface(session);
}

async function readModelSurface(session: RenderedPlantSession): Promise<any> {
  return JSON.parse(await session.inner("var text=(value)=>(value||'').replace(/\\s+/g,' ').trim();var model=d.querySelector('[data-plant-setup-section=model-behavior]');var starting=d.querySelector('[data-plant-setup-section=starting-conditions]');var diagram=model&&model.querySelector('[data-plant-model-diagram]');var labels=diagram?[...diagram.querySelectorAll('svg text')].map((node)=>({text:text(node.textContent),rect:node.getBoundingClientRect()})):[];var textCollisions=[];for(var i=0;i<labels.length;i+=1){for(var j=i+1;j<labels.length;j+=1){var a=labels[i],b=labels[j];if(a.rect.left<b.rect.right-1&&a.rect.right>b.rect.left+1&&a.rect.top<b.rect.bottom-1&&a.rect.bottom>b.rect.top+1)textCollisions.push([a.text,b.text]);}}var dimensions=(name)=>model?.querySelector('[data-plant-state-space-dimension='+name+']')?.value;var stateSpace=model?.querySelector('[data-plant-state-space-editor]')?{states:dimensions('states'),inputs:dimensions('inputs'),outputs:dimensions('outputs'),matrices:[...model.querySelectorAll('[data-plant-matrix-cell]')].map((field)=>field.getAttribute('data-plant-matrix-cell')),initial:model.querySelectorAll('[data-plant-initial-state-cell]').length}:undefined;var ports=[...d.querySelectorAll('select[data-plant-port-direction]')].map((select)=>text(select.labels?.[0]?.querySelector('span')?.textContent));return JSON.stringify({explanation:text(model?.querySelector('[data-plant-model-explanation]')?.textContent),diagram:{kind:diagram?.getAttribute('data-plant-model-diagram')||'',role:diagram?.getAttribute('role')||'',label:diagram?.getAttribute('aria-label')||'',caption:text(diagram?.querySelector('figcaption')?.textContent),textCollisions},dynamics:[...model.querySelectorAll('[data-plant-dynamics-field]')].map((field)=>field.getAttribute('data-plant-dynamics-field')),starting:[...starting.querySelectorAll('[data-plant-starting-state],[data-plant-dynamics-field=starting_value]')].map((field)=>text(field.closest('label')?.querySelector('span')?.textContent)),ports,piecewiseRows:model.querySelectorAll('[data-plant-piecewise-row]').length,stateSpace,temperatureExample:Boolean(model.querySelector('[data-plant-temperature-example]')),manualOpen:d.querySelectorAll('[data-plant-manual-address-toggle][aria-expanded=true],[data-plant-manual-address-input]').length});"));
}

async function saveAndReopenPlant(session: RenderedPlantSession): Promise<void> {
  const clicked = await session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');var save=panel&&[...panel.querySelectorAll('button')].find((button)=>(button.textContent||'').trim()==='Save');if(!save||save.disabled)return 'NOT_READY';save.click();return 'SAVED';");
  assert.strictEqual(clicked, "SAVED", "the valid Piecewise linear route should be ready for a real save");
  await pollValue("saved Piecewise linear draft to close", () => session.inner("return d.querySelector('aside[aria-label=\"Plant properties\"]')?'OPEN':'CLOSED';"), (value) => value === "CLOSED", 20_000);
  await pollValue("persisted Piecewise linear node to reopen", () => session.inner("var node=[...d.querySelectorAll('.react-flow__node')].find((candidate)=>(candidate.textContent||'').includes('New simulated plant')&&!(candidate.textContent||'').includes('DRAFT'));if(!node)return 'WAIT';node.dispatchEvent(new node.ownerDocument.defaultView.MouseEvent('click',{bubbles:true}));return 'CLICKED';"), (value) => value === "CLICKED", 20_000);
  await pollValue("persisted Piecewise linear editor", () => session.inner("var panel=d.querySelector('aside[aria-label=\"Plant properties\"]');return (panel?.textContent||'').includes('Configure plant')?'OPEN':'WAIT';"), (value) => value === "OPEN", 20_000);
}

async function captureModelEvidence(session: RenderedPlantSession, route: ModelRoute): Promise<void> {
  const base = process.env.TRUST_PLANT_RENDERED_SCREENSHOT?.trim();
  const selected = process.env.TRUST_PLANT_RENDERED_CAPTURE_MODEL;
  if (!base || selected && selected !== route.kind) return;
  const screenshot = selected ? base : base.replace(/\.png$/i, `-${route.kind}.png`);
  const surface = JSON.parse(await session.inner("var panel=d.querySelector('[data-plant-layout=main-canvas]');var model=d.querySelector('[data-plant-setup-section=model-behavior]');if(panel)panel.scrollTop=0;var rect=panel?.getBoundingClientRect();var diagram=model?.querySelector('[data-plant-model-diagram]');var diagramRect=diagram?.getBoundingClientRect();return JSON.stringify({clientWidth:panel?.clientWidth,scrollWidth:panel?.scrollWidth,overflowX:Boolean(panel&&panel.scrollWidth>panel.clientWidth+1),panel:{left:rect&&Math.round(rect.left),right:rect&&Math.round(rect.right)},diagram:{left:diagramRect&&Math.round(diagramRect.left),right:diagramRect&&Math.round(diagramRect.right),width:diagramRect&&Math.round(diagramRect.width)}});"));
  assert.strictEqual(surface.overflowX, false, `${route.name} model route must not overflow horizontally: ${JSON.stringify(surface)}`);
  await sleep(250);
  captureRenderedWorkbench(screenshot);
  const evidencePath = screenshot.toLowerCase().endsWith(".png") ? `${screenshot.slice(0, -4)}.json` : `${screenshot}.json`;
  fs.writeFileSync(evidencePath, JSON.stringify({
    model: route.kind,
    explanation: route.explanation,
    split: process.env.TRUST_PLANT_RENDERED_SPLIT === "1",
    surface,
    rendered: await readModelSurface(session),
  }, null, 2));
}
