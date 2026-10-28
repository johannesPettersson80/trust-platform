import * as assert from "assert";
import { formatEngineeringNumber } from "../../controlStudio/chartContract";
import type { LoopDefinition } from "../../controlStudio/generated/OpenResult";
import { tuneCandidatePayload } from "../../controlStudio/webview/engineTune";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { validateReceiver } from "../../controlStudio/webview/engineValidate";
import { cascadeLoop, gainScheduleLoop, onOffLoop, pidGroupLoop, smithLibraryLoop, smithLoop } from "./control-studio-structure-loop-fixtures";

/**
 * The Tune result and the Validate rows of the five structures the engine serves without a lesson (specification 35 C3, C6, C7, C9 and C10).
 * Each page section is rendered from a fixture answer in the shape the engine gives (TuneResult, RunRequirementResult); the loops are the
 * fixtures of control-studio-structure-loop-fixtures.ts. The page keeps no rule of its own, so each assertion reads a value the answer or the
 * loop holds.
 */

type Loop = Record<string, any>;

/** The engine's words of a Tune answer for the gain names the structure tables show (`words.gainLabels`, TuneWords). */
const STRUCTURE_WORDS = { gainLabels: { proportional: "Proportional gain (Kp)", integral: "Integral gain (Ki)", derivative: "Derivative gain (Kd)" } };

/** Runs `check` with a fresh DOM as the global document, as the other page tests do; the body holds a tuning area and a validation table. */
function withPage<T>(check: (body: any) => T): T {
  const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };
  const dom = new JSDOM(
    "<main><div data-studio-tuning-result hidden></div><table data-studio-validation-result hidden><tbody></tbody></table></main>",
  );
  const saved = (globalThis as any).document;
  (globalThis as any).document = dom.window.document;
  try {
    return check(dom.window.document.body);
  } finally {
    (globalThis as any).document = saved;
  }
}

function loopOf(build: () => Record<string, unknown>): LoopDefinition {
  return build() as unknown as LoopDefinition;
}

/** The text of each row's cells, in order. */
function cellTexts(table: any): string[][] {
  return [...table.tBodies[0].rows].map((row: any) => [...row.cells].map((cell: any) => cell.textContent ?? ""));
}

/** The binding's parameter that holds the given gain (its convention names the gain: `Control.PID ProportionalGain`). */
function gainSymbolOf(loop: Loop, symbols: readonly string[], gain: "ProportionalGain" | "IntegralGain"): string {
  const parameter = (loop.parameterBinding?.parameters as { symbol: string; convention: string }[])
    .find(entry => symbols.includes(entry.symbol) && entry.convention.includes(gain));
  assert.ok(parameter, `the loop binds a ${gain} for ${symbols.join(", ")}`);
  return parameter.symbol;
}

suite("Control Studio structure results on the Tune page", () => {
  test("a cascade shows the inner PI and the outer PI, inner first, with each instance and its gains", () => {
    const loop: Loop = loopOf(cascadeLoop) as unknown as Loop;
    const seed = loop.controller.project.seed as { inner: { instance: string; proportionalSymbol: string; integralSymbol: string };
      outer: { instance: string; proportionalSymbol: string; integralSymbol: string } };
    const values = {
      [seed.inner.proportionalSymbol]: 2.5, [seed.inner.integralSymbol]: 0.8,
      [seed.outer.proportionalSymbol]: 1.2, [seed.outer.integralSymbol]: 0.3,
    };
    withPage(body => {
      renderTunedController(body, loop as unknown as LoopDefinition, { status: "candidate", words: STRUCTURE_WORDS, family: "PI", values });
      assert.strictEqual(body.querySelector("p").textContent, "Cascade");
      const table = body.querySelector("[data-studio-cascade-design]");
      assert.ok(table, "the cascade's table is shown");
      const rows = cellTexts(table);
      assert.strictEqual(rows.length, 2, "one row for each controller");
      assert.strictEqual(rows[0][0], `Inner · ${seed.inner.instance}`);
      assert.strictEqual(rows[1][0], `Outer · ${seed.outer.instance}`);
      assert.ok(rows[0][1].startsWith(formatEngineeringNumber(2.5)), "the inner proportional gain is the answer's");
      assert.ok(rows[0][2].startsWith(formatEngineeringNumber(0.8)), "the inner integral gain is the answer's");
      assert.ok(rows[1][1].startsWith(formatEngineeringNumber(1.2)), "the outer proportional gain is the answer's");
      assert.ok(rows[1][2].startsWith(formatEngineeringNumber(0.3)), "the outer integral gain is the answer's");
      assert.strictEqual(body.querySelectorAll("[data-studio-parameter]").length, 0, "the PID rows are not drawn for a cascade");
    });
  });

  test("the gain names of a structure's table are the engine's words, not the page's copy", () => {
    const loop: Loop = loopOf(cascadeLoop) as unknown as Loop;
    const seed = loop.controller.project.seed as { inner: { proportionalSymbol: string; integralSymbol: string } };
    const values = { [seed.inner.proportionalSymbol]: 2.5, [seed.inner.integralSymbol]: 0.8 };
    const words = { gainLabels: { proportional: "Engine proportional term", integral: "Engine integral term", derivative: "Engine derivative term" } };
    withPage(body => {
      renderTunedController(body, loop as unknown as LoopDefinition, { status: "candidate", family: "PI", values, words });
      const header = body.querySelector("[data-studio-cascade-design] thead").textContent;
      assert.strictEqual(header, "ControllerEngine proportional termEngine integral term", "the header names the gains as the engine words them");
    });
  });

  test("a Smith predictor shows its PI and the model's gain, time constant and delay", () => {
    const loop = loopOf(smithLoop);
    const seed = (loop as unknown as Loop).controller.project.seed as { proportionalSymbol: string; integralSymbol: string;
      gainSymbol: string; timeConstantSymbol: string; delaySymbol: string };
    const values = { [seed.proportionalSymbol]: 3, [seed.integralSymbol]: 0.4, [seed.gainSymbol]: 0.2, [seed.timeConstantSymbol]: 5, [seed.delaySymbol]: 1 };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "PI", values });
      assert.strictEqual(body.querySelector("p").textContent, "Smith predictor");
      const list = body.querySelector("[data-studio-smith-design]");
      assert.ok(list, "the predictor's section is shown");
      assert.deepStrictEqual([...list.querySelectorAll("dt")].map((term: any) => term.textContent),
        ["Proportional gain (Kp)", "Integral gain (Ki)", "Model gain K", "Model time constant T", "Model delay θ"]);
      const detail = list.querySelector(`dd[data-studio-parameter="${seed.gainSymbol}"]`);
      assert.ok(detail?.textContent?.startsWith(formatEngineeringNumber(0.2)), "the model gain is the answer's");
    });
  });

  test("a Smith predictor with the library block also shows the operating point", () => {
    const loop = loopOf(smithLibraryLoop);
    const seed = (loop as unknown as Loop).controller.project.seed as { proportionalSymbol: string; integralSymbol: string;
      gainSymbol: string; timeConstantSymbol: string; delaySymbol: string; operatingInputSymbol: string; operatingOutputSymbol: string };
    const values = { [seed.proportionalSymbol]: 3, [seed.integralSymbol]: 0.4, [seed.gainSymbol]: 0.2, [seed.timeConstantSymbol]: 5,
      [seed.delaySymbol]: 1, [seed.operatingInputSymbol]: 40, [seed.operatingOutputSymbol]: 22 };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "PI", values });
      const terms = [...body.querySelector("[data-studio-smith-design]").querySelectorAll("dt")].map((term: any) => term.textContent);
      assert.deepStrictEqual(terms.slice(5), ["Operating input", "Operating output"]);
      const input = body.querySelector(`dd[data-studio-parameter="${seed.operatingInputSymbol}"]`);
      assert.ok(input?.textContent?.startsWith(formatEngineeringNumber(40)), "the operating input is the answer's");
    });
  });

  test("an ON/OFF controller shows its hysteresis, its minimum times and the declared output levels", () => {
    const loop = loopOf(onOffLoop);
    const block = (loop as unknown as Loop).controller.onOff;
    const values = { [block.hysteresisSymbol]: 0.8, [block.minimumOnTimeSymbol]: 20, [block.minimumOffTimeSymbol]: 30 };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "", values });
      assert.strictEqual(body.querySelector("p").textContent, "ON/OFF controller");
      const list = body.querySelector("[data-studio-on-off-design]");
      assert.ok(list, "the switching section is shown");
      assert.deepStrictEqual([...list.querySelectorAll("dt")].map((term: any) => term.textContent),
        ["Hysteresis", "Minimum ON time", "Minimum OFF time", "Output levels"]);
      assert.ok(list.querySelector(`dd[data-studio-parameter="${block.hysteresisSymbol}"]`)?.textContent?.startsWith(formatEngineeringNumber(0.8)));
      assert.ok(list.querySelector(`dd[data-studio-parameter="${block.minimumOnTimeSymbol}"]`)?.textContent?.startsWith(formatEngineeringNumber(20)));
      assert.ok(list.querySelector(`dd[data-studio-parameter="${block.minimumOffTimeSymbol}"]`)?.textContent?.startsWith(formatEngineeringNumber(30)));
      const levels = list.querySelector("[data-studio-on-off-levels]")?.textContent ?? "";
      assert.ok(levels.includes(`OFF at ${formatEngineeringNumber(block.lowOutput)}`), "the OFF level is the declared one");
      assert.ok(levels.includes(`ON at ${formatEngineeringNumber(block.highOutput)}`), "the ON level is the declared one");
    });
  });

  test("a PID group shows each member's PI by its paired channels, and the decoupler's coefficients", () => {
    const loop = loopOf(pidGroupLoop);
    const declared = ((loop as unknown as Loop).controller.group.members as { instance: string; inputChannelId: string;
      outputChannelId: string; parameterSymbols: string[] }[]);
    // The engine names each channel (GroupMemberDesign.inputName, outputName; GroupDecoupler.inputNames): the page resolves no signal from the loop.
    const members = declared.map((member, index) => ({
      instance: member.instance,
      inputChannelId: member.inputChannelId,
      outputChannelId: member.outputChannelId,
      inputName: `input-name-${index}`,
      outputName: `output-name-${index}`,
      relativeGain: 1.1,
      proportionalSymbol: gainSymbolOf(loop as unknown as Loop, member.parameterSymbols, "ProportionalGain"),
      integralSymbol: gainSymbolOf(loop as unknown as Loop, member.parameterSymbols, "IntegralGain"),
      gains: { proportionalGain: 2 + index, integralGain: 0.5 + index },
    }));
    const matrix = members.map((_, row) => members.map((_, column) => (row === column ? 1.1 : -0.34)));
    const design = {
      group: {
        applied: false,
        source: "SIMC",
        members,
        decoupler: { matrix, inputChannelIds: members.map(member => member.inputChannelId), inputNames: members.map(member => member.inputName),
          memberInstances: members.map(member => member.instance) },
        values: {},
      },
    };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "", values: {}, design });
      assert.strictEqual(body.querySelector("p").textContent, "PID group");
      const rows = cellTexts(body.querySelector("[data-studio-group-members]"));
      assert.strictEqual(rows.length, members.length, "one row for each member");
      rows.forEach((row, index) => {
        assert.strictEqual(row[0], members[index].instance);
        assert.strictEqual(row[1], `input-name-${index} → output-name-${index}`, "the member is shown by its paired channels, named as the engine names them");
        assert.ok(row[2].startsWith(formatEngineeringNumber(2 + index)), "the member's proportional gain is the answer's");
        assert.ok(row[3].startsWith(formatEngineeringNumber(0.5 + index)), "the member's integral gain is the answer's");
        assert.strictEqual(row[4], formatEngineeringNumber(1.1));
      });
      const decoupler = cellTexts(body.querySelector("[data-studio-group-decoupler]"));
      assert.strictEqual(decoupler.length, members.length, "one row for each plant input");
      assert.strictEqual(decoupler[0][0], "input-name-0", "each row of D is named as the engine names the plant input");
      assert.strictEqual(decoupler[0][1], formatEngineeringNumber(1.1));
      assert.strictEqual(decoupler[0][2], formatEngineeringNumber(-0.34));
    });
    // A candidate the study kept before the engine named the channels names none: the channel identifiers stand, and the page looks nothing up.
    const unnamed = {
      group: { ...design.group, members: members.map(({ inputName: _input, outputName: _output, ...rest }) => rest),
        decoupler: { matrix, inputChannelIds: members.map(member => member.inputChannelId), memberInstances: members.map(member => member.instance) } },
    };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "", values: {}, design: unnamed });
      const rows = cellTexts(body.querySelector("[data-studio-group-members]"));
      rows.forEach((row, index) => assert.strictEqual(row[1], `${members[index].inputChannelId} → ${members[index].outputChannelId}`));
      assert.strictEqual(cellTexts(body.querySelector("[data-studio-group-decoupler]"))[0][0], members[0].inputChannelId);
    });
  });

  test("a gain schedule the engine did not schedule says the fixed design holds", () => {
    const loop = loopOf(gainScheduleLoop);
    const points = ((loop as unknown as Loop).controller.project.schedule.points as { name: string; value: number;
      proportionalSymbol: string; integralSymbol: string }[]);
    const first = points[0];
    const schedule = {
      choice: "fixed",
      reason: "Fixed design: one PI for every operating point meets its requirements.",
      scheduling: { symbol: "P1.Temperature", unit: "°C" },
      points: points.map(point => ({ ...point, proportionalGain: 1.4, integralGain: 0.2 })),
    };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "PI", values: {}, design: { schedule } });
      assert.strictEqual(body.querySelector("p").textContent, "Gain schedule");
      const sentence = body.querySelector("[data-studio-schedule-choice]");
      assert.strictEqual(sentence?.getAttribute("data-studio-schedule-choice"), "fixed");
      assert.strictEqual(sentence?.textContent, schedule.reason);
      assert.strictEqual(body.querySelector("[data-studio-schedule-table]"), null, "no table is shown for the fixed design");
      const fixed = body.querySelector("[data-studio-schedule-fixed]");
      assert.ok(fixed?.querySelector(`dd[data-studio-parameter="${first.proportionalSymbol}"]`)?.textContent?.startsWith(formatEngineeringNumber(1.4)));
    });
  });

  test("a scheduled gain schedule shows each operating point with its scheduling value and its gains", () => {
    const loop = loopOf(gainScheduleLoop);
    const points = ((loop as unknown as Loop).controller.project.schedule.points as { name: string; value: number;
      proportionalSymbol: string; integralSymbol: string }[]);
    const schedule = {
      choice: "scheduled",
      reason: "Scheduled design: the fixed design fails a requirement at operating point Warm; each point has its own PI.",
      scheduling: { symbol: "P1.Temperature", unit: "°C" },
      points: points.map((point, index) => ({ ...point, proportionalGain: 1.4 + index, integralGain: 0.2 + index })),
    };
    withPage(body => {
      renderTunedController(body, loop, { status: "candidate", words: STRUCTURE_WORDS, family: "PI", values: {}, design: { schedule } });
      assert.strictEqual(body.querySelector("[data-studio-schedule-choice]")?.getAttribute("data-studio-schedule-choice"), "scheduled");
      const rows = cellTexts(body.querySelector("[data-studio-schedule-table]"));
      assert.strictEqual(rows.length, points.length, "one row for each operating point");
      rows.forEach((row, index) => {
        assert.strictEqual(row[0], points[index].name);
        assert.ok(row[1].startsWith(formatEngineeringNumber(points[index].value)), "the scheduling value is the answer's");
        assert.ok(row[2].startsWith(formatEngineeringNumber(1.4 + index)), "the point's proportional gain is the answer's");
        assert.ok(row[3].startsWith(formatEngineeringNumber(0.2 + index)), "the point's integral gain is the answer's");
      });
    });
  });

  test("the Validate rows of a switching controller and of a Smith predictor show the engine's words", () => {
    // The engine's table holds these rows (ValidateResult.table); the page writes each row's words and its verdict sentence as given.
    withPage(body => {
      validateReceiver.result!(body, { applyAvailable: false, table: [
        { title: "Switching at threshold", symbol: "", source: "", measured: "49.8 °C", limit: `threshold ${formatEngineeringNumber(50)} °C`, verdict: "fail",
          verdictText: "Fail · A change at 49.8 °C did not happen at its threshold (C3).", verdictClass: "fail", requirement: "switching-transitions" },
        { title: "Retained band", symbol: "", source: "", measured: "—", limit: "—", verdict: "pass",
          verdictText: "Pass · The requested state stayed inside its band at every scan (C3).", verdictClass: "pass", requirement: "switching-band" },
        { title: "Output aborts", symbol: "", source: "", measured: "0 count", limit: "≤ 0 count", verdict: "pass",
          verdictText: "Pass · No abort: every change of the applied output followed its requested state (C3).", verdictClass: "pass", requirement: "switching-aborts" },
        { title: "Predictor active", symbol: "", source: "", measured: `${formatEngineeringNumber(12.5)} s`, limit: "—", verdict: "fail",
          verdictText: "Fail · The predictor's history is incomplete at 12.500 s, so the predictor is not active.", verdictClass: "fail", requirement: "smith-predictor" },
      ] }, { receive: () => undefined } as any);
      const row = (id: string) => body.querySelector(`[data-studio-requirement="${id}"]`);
      const transitions = row("switching-transitions");
      assert.ok(transitions, "the transition row is shown");
      assert.strictEqual(transitions.cells[0].textContent, "Switching at threshold", "the label is the engine's words");
      assert.strictEqual(transitions.cells[2].textContent, `threshold ${formatEngineeringNumber(50)} °C`);
      assert.ok(transitions.textContent.includes("A change at 49.8 °C did not happen at its threshold (C3)."), "the verdict shows the engine's sentence");
      assert.strictEqual(row("switching-band")?.cells[0].textContent, "Retained band");
      assert.strictEqual(row("switching-aborts")?.cells[0].textContent, "Output aborts");
      const predictor = row("smith-predictor");
      assert.ok(predictor, "the predictor row is shown");
      assert.strictEqual(predictor.cells[0].textContent, "Predictor active");
      assert.ok(predictor.cells[1].textContent.startsWith(formatEngineeringNumber(12.5)), "the predictor row shows the time of the failure");
    });
  });

  test("the engine's answer carries a group and a schedule to the page's section", () => {
    const group = { applied: false, source: "SIMC", members: [], values: {} };
    const schedule = { choice: "fixed", reason: "Fixed design.", scheduling: { symbol: "P1.Temperature", unit: "°C" }, points: [] };
    const withGroup = tuneCandidatePayload({ projectCandidate: { applied: false, source: "SIMC", structure: "PI", gains: {}, values: {} }, group });
    assert.deepStrictEqual((withGroup.design as { group: unknown }).group, group, "a group's answer goes under design.group");
    const withSchedule = tuneCandidatePayload({ projectCandidate: { applied: false, source: "SIMC", structure: "PI", gains: {}, values: {} }, schedule });
    assert.deepStrictEqual((withSchedule.design as { schedule: unknown }).schedule, schedule, "a schedule's answer goes under design.schedule");
    const plain = tuneCandidatePayload({ projectCandidate: { applied: false, source: "SIMC", structure: "PI", gains: {}, values: {} } });
    assert.strictEqual(plain.design, undefined, "a program's law without a group or a schedule has no design section");
  });
});
