import * as assert from "assert";
import type { EvaluateWebview } from "./control-studio-navigation-assertions";

/**
 * Exercise the real Verify Settings scenario editor without leaving a draft
 * behind. The capture is taken while the newly populated scenario is visible.
 */
export async function exerciseControlStudioScenarioSettings(
  inner: EvaluateWebview,
  capture: (suffix: string) => Promise<void>,
): Promise<void> {
  const before = JSON.parse(await inner(`return JSON.stringify((() => {
    const root = d.querySelector('[data-engineer-workflow]');
    const raw = d.querySelector('[data-engineer-scenarios]');
    const options = d.querySelector('[data-engineer-options]');
    const details = d.querySelector('[data-engineer-results]');
    return {
      stage: root?.dataset.selectedStage ?? null,
      options: Boolean(options?.open),
      details: Boolean(details?.open),
      raw: raw?.value ?? null,
    };
  })());`));

  try {
    await inner(`
      const tab = d.querySelector('[data-engineer-stage-tab="verify"]');
      if (!tab) throw new Error('Verify stage tab is missing');
      tab.click();
      const options = d.querySelector('[data-engineer-options]');
      if (!options) throw new Error('Settings inspector is missing');
      options.open = true;
      return 'verify settings';
    `);

    const originalScenarioJson = JSON.parse(before.raw ?? "[]");
    await inner(`
      const add = d.querySelector('[data-engineer-scenario-add]');
      if (!add) throw new Error('Add scenario control is missing');
      add.click();
      return 'scenario added';
    `);

    const nameResult = JSON.parse(await inner(`return JSON.stringify((() => {
      const name = [...d.querySelectorAll('[data-engineer-scenario-id]')].at(-1);
      if (!name) throw new Error('New scenario name field is missing');
      const node = name;
      node.focus();
      node.value = '';
      for (const character of 'step-response') {
        node.value += character;
        node.dispatchEvent(new d.defaultView.Event('input', { bubbles: true }));
        if (d.querySelector('[data-engineer-scenario-id]:last-of-type') !== node &&
            [...d.querySelectorAll('[data-engineer-scenario-id]')].at(-1) !== node) {
          throw new Error('Scenario name node was replaced while typing');
        }
      }
      return { value: node.value, focused: d.activeElement === node, caret: node.selectionStart };
    })());`));
    assert.deepStrictEqual(nameResult, { value: "step-response", focused: true, caret: "step-response".length },
      "typing the scenario name must preserve the live field and focus");

    await inner(`
      const addValue = [...d.querySelectorAll('[data-engineer-scenario-add-value]')].at(-1);
      if (!addValue) throw new Error('Add initial value control is missing');
      addValue.click();
      return 'initial value added';
    `);

    const valueResult = JSON.parse(await inner(`return JSON.stringify((() => {
      const symbol = [...d.querySelectorAll('[data-engineer-scenario-initial-symbol]')].at(-1);
      const value = [...d.querySelectorAll('[data-engineer-scenario-initial]')].at(-1);
      const raw = d.querySelector('[data-engineer-scenarios]');
      if (!symbol || !value || !raw) throw new Error('Initial value controls are missing');
      const symbolNode = symbol;
      symbolNode.focus();
      symbolNode.value = '';
      for (const character of 'Main.SP') {
        symbolNode.value += character;
        symbolNode.dispatchEvent(new d.defaultView.Event('input', { bubbles: true }));
        if ([...d.querySelectorAll('[data-engineer-scenario-initial-symbol]')].at(-1) !== symbolNode) {
          throw new Error('Initial signal node was replaced while typing');
        }
      }
      const valueNode = value;
      valueNode.focus();
      valueNode.value = '0.2';
      valueNode.dispatchEvent(new d.defaultView.Event('input', { bubbles: true }));
      const scenarios = JSON.parse(raw.value);
      const scenario = scenarios.at(-1);
      return {
        symbol: symbolNode.value,
        value: valueNode.value,
        focused: d.activeElement === valueNode,
        symbolCaret: symbolNode.selectionStart,
        valueCaret: valueNode.selectionStart,
        initialValues: scenario?.initialValues,
        rawScenario: scenario,
      };
    })());`));
    assert.strictEqual(valueResult.symbol, "Main.SP");
    assert.strictEqual(valueResult.value, "0.2");
    assert.strictEqual(valueResult.focused, true,
      "editing the initial value must preserve the live field and focus");
    assert.strictEqual(valueResult.symbolCaret, "Main.SP".length,
      "typing the initial signal must preserve the caret at the end of the field");
    assert.strictEqual(valueResult.valueCaret, null, "Numeric controls do not expose a text selection caret");
    assert.deepStrictEqual(valueResult.initialValues, { "Main.SP": 0.2 },
      "the visible initial value editor must update the raw scenario definition");
    assert.strictEqual(valueResult.rawScenario.id, "step-response");
    assert.deepStrictEqual(JSON.parse(await inner("return d.querySelector('[data-engineer-scenarios]').value;" )).at(-1).initialValues,
      { "Main.SP": 0.2 }, "the raw JSON must retain the edited Main.SP value");

    await capture("-scenario-populated-settings");

    await inner(`
      const cards = [...d.querySelectorAll('[data-engineer-scenario]')];
      const card = cards.at(-1);
      const remove = card?.querySelector('[data-engineer-scenario-remove]');
      if (!remove) throw new Error('New scenario remove control is missing');
      remove.click();
      return 'scenario removed';
    `);
    assert.deepStrictEqual(JSON.parse(await inner("return d.querySelector('[data-engineer-scenarios]').value;")), originalScenarioJson,
      "removing the temporary scenario must restore the prior scenario definitions");
  } finally {
    await inner(`
      const raw = d.querySelector('[data-engineer-scenarios]');
      const originalRaw = ${JSON.stringify(before.raw)};
      if (raw && raw.value !== originalRaw) {
        raw.value = originalRaw;
        raw.dispatchEvent(new d.defaultView.Event('input', { bubbles: true }));
      }
      const stage = ${JSON.stringify(before.stage)};
      if (stage) d.querySelector('[data-engineer-stage-tab="' + stage + '"]')?.click();
      const options = d.querySelector('[data-engineer-options]');
      const details = d.querySelector('[data-engineer-results]');
      if (options) options.open = false;
      if (details) details.open = false;
      if (${Boolean(before.options)}) options && (options.open = true);
      else if (${Boolean(before.details)}) details && (details.open = true);
      return 'settings interaction restored';
    `);
  }
}
