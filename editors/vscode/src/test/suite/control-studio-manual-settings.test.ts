import * as assert from "assert";
import { renderTunedController } from "../../controlStudio/webview/threePageResults";
import { initializeManualTuneSettings } from "../../controlStudio/webview/manualTuneSettings";
import { setOpenedOffers } from "../../controlStudio/webview/openedOffers";

const { JSDOM } = require("jsdom") as { JSDOM: new (html: string) => any };

/** A setting row as the engine answers it (`controllerOffer.settingRows`, C2): its symbol, its label, its unit and its bounds. */
interface SettingRow { symbol: string; label: string; unit: string; minimum: number; maximum: number }

/** The three PID gains, labelled as the engine labels them. */
const PID_ROWS: SettingRow[] = [
  { symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)", unit: "%/degC", minimum: 0, maximum: 100 },
  { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)", unit: "%/(degC*s)", minimum: 0, maximum: 100 },
  { symbol: "P1.Parameters.DerivativeGain", label: "Derivative gain (Kd)", unit: "%*s/degC", minimum: 0, maximum: 100 },
];

/** Gives the page the rows the engine answered for the loop's settings, as the open answer does (`controllerOffer`, C2). */
function offerRows(rows: readonly SettingRow[]): void {
  setOpenedOffers({ controllerOffer: { families: [], familySettings: {}, settingRows: [...rows] } } as any);
}

const loop = {
  controller: {
    instance: "P1.Controller",
    type: "Control.PID",
    law: {
      kind: "pid",
      parameterRecordSymbol: "P1.Parameters",
      parameterRevisionSymbol: "P1.Parameters.Revision",
      proportionalGainSymbol: "P1.Parameters.ProportionalGain",
      integralGainSymbol: "P1.Parameters.IntegralGain",
      derivativeGainSymbol: "P1.Parameters.DerivativeGain",
    },
  },
  parameterBinding: {
    parameters: [
      { symbol: "P1.Parameters.ProportionalGain", iec_type: "LREAL", unit: "%/degC", convention: "Kp", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.IntegralGain", iec_type: "LREAL", unit: "%/(degC*s)", convention: "Ki", minimum: 0, maximum: 100 },
      { symbol: "P1.Parameters.DerivativeGain", iec_type: "LREAL", unit: "%*s/degC", convention: "Kd", minimum: 0, maximum: 100 },
    ],
  },
} as any;

suite("Control Studio manual Tune settings", () => {
  test("labels a pinned finite design without presenting nominal Riccati gains as its active policy", () => {
    const f = setup({ status: "candidate", family: "LQR", values: {} });
    try {
      // The line and the heading are the engine's words (TuneWords.horizonPolicy, horizonStages): a pin fixes the gains, so the stages are the nominal ones.
      renderTunedController(f.root, loop, { status: "candidate", family: "LQR", values: {},
        words: { outcome: "Meets the requirements", horizonPolicy: "Finite horizon: 1 step · Pinned fixed gain", horizonStages: "Nominal Riccati stages" },
        state: { family: "LQR", horizon: { steps: 1, policy: "pinned-fixed-gain", gainSequence: [[[.5]]], riccatiSequence: [[[1.5]], [[1]]] } } });
      assert.strictEqual(f.root.querySelector("[data-studio-design-policy]")!.textContent, "Finite horizon: 1 step · Pinned fixed gain");
      assert.strictEqual(f.root.querySelector("details summary")!.textContent, "Nominal Riccati stages");
      assert.strictEqual(f.root.querySelector<HTMLDetailsElement>("details")!.open, false);
    } finally { f.restore(); }
  });
  test("shows a dimensionless tuned value without a unit", () => {
    const lqg = { controller: { instance: "P1", type: "LQG state feedback" }, parameterBinding: { parameters: [
      { symbol: "P1.K1", iec_type: "LREAL", unit: "m3/s/m", convention: "Gain K1", minimum: -1, maximum: 1 },
      { symbol: "P1.L1", iec_type: "LREAL", unit: "1", convention: "Estimator gain L1", minimum: -5, maximum: 5 }] } } as any;
    const f = setup({ status: "candidate", family: "LQG", values: {} });
    try {
      renderTunedController(f.root, lqg, { status: "candidate", family: "LQG", values: { "P1.K1": 0.07874, "P1.L1": 0.2393 },
        words: { outcome: "No prediction: The study keeps no prediction.", settingLabels: [{ symbol: "P1.K1", label: "Gain K1", unit: "m3/s/m" },
          { symbol: "P1.L1", label: "Estimator gain L1", unit: "1" }] } });
      assert.strictEqual(f.root.querySelector("[data-studio-parameter='P1.L1']")!.textContent, "0.2393");
      assert.match(f.root.querySelector("[data-studio-parameter='P1.K1']")!.textContent!, /^0\.0787 \S/);
    } finally { f.restore(); }
  });

  test("shows no manual row for an answer that names no family: the page invents none, where it assumed PID before", () => {
    const f = setup({ status: "candidate", values: { "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5 } });
    try {
      assert.strictEqual(f.root.querySelectorAll("[data-studio-manual-value]").length, 0, "no family, no rows");
      assert.strictEqual(f.root.querySelector("[data-studio-tuning-result] p")!.textContent, "", "and no family is named in the title");
    } finally { f.restore(); }
  });

  test("words a finite horizon only as the engine words it: an answer without the words shows none of its own", () => {
    // The page kept a table of policy words and wrote "Repeated first gain" for any policy but the pinned one. The engine words both policies
    // (tune_words_tests.rs, every_offered_choice_carries_its_words), so an answer without the words shows the stage table under no invented name.
    const f = setup({ status: "candidate", family: "LQR", values: {} });
    try {
      renderTunedController(f.root, loop, { status: "candidate", family: "LQR", values: {},
        state: { family: "LQR", horizon: { steps: 2, policy: "repeat-first-finite-gain", gainSequence: [[[.5]], [[.4]]], riccatiSequence: [[[1.5]], [[1.2]], [[1]]] } } });
      assert.strictEqual(f.root.querySelector("[data-studio-design-policy]")!.textContent, "");
      assert.strictEqual(f.root.querySelector("details summary")!.textContent, "");
    } finally { f.restore(); }
  });

  test("shows every Riccati stage's gain and cost with one precision per matrix", () => {
    const f = setup({ status: "candidate", family: "LQR", values: {} });
    try {
      renderTunedController(f.root, loop, { status: "candidate", family: "LQR", values: {},
        state: { family: "LQR", horizon: { steps: 2, policy: "repeat-first-finite-gain", gainSequence: [[[0.07841259, 0.02451445]], [[0.01815348, 0.00036036]]],
          riccatiSequence: [[[1948.2568, 554.3539], [554.3539, 4065.8254]], [[756.1485, 14.4981], [14.4981, 769.2411]], [[400, 0], [0, 400]]] } } });
      const rows = [...f.root.querySelectorAll("[data-studio-riccati-stage]")].map(row => [...(row as HTMLTableRowElement).cells].map(cell => cell.textContent));
      assert.deepStrictEqual(rows, [
        ["0", "[0.0784, 0.0245]", "[1948.3, 554.4]; [554.4, 4065.8]"],
        ["1", "[0.018153, 0.00036]", "[756.15, 14.5]; [14.5, 769.24]"],
        ["2", "—", "[400, 0]; [0, 400]"],
      ], "Equal entries read alike and no entry switches to exponent notation");
    } finally { f.restore(); }
  });
  /** Renders the candidate's manual settings from the engine's rows, and the symbols the engine names for each family (C2). */
  function setup(payload: Record<string, unknown>, rows: readonly SettingRow[] = PID_ROWS,
    familySymbols: (family: string) => readonly string[] | undefined = () => rows.map(row => row.symbol)) {
    offerRows(rows);
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    const root = dom.window.document.querySelector("main") as HTMLElement;
    const controls = initializeManualTuneSettings(root, undefined, familySymbols);
    controls.render(payload);
    return { dom, root, controls, restore() { setOpenedOffers({}); (globalThis as any).document = previousDocument; (globalThis as any).window = previousWindow; dom.window.close(); } };
  }

  test("names a project controller's gains in Manual as the engine's rows name them", () => {
    const symbols = ["P1.Kp", "P1.Ki", "P1.Kd", "P1.Kv", "P1.Ka"];
    const labels = ["Proportional gain (Kp)", "Integral gain (Ki)", "Derivative gain (Kd)", "Rate feedforward (Kv)", "Acceleration feedforward (Ka)"];
    const rows = symbols.map((symbol, index) => ({ symbol, label: labels[index], unit: "1", minimum: 0, maximum: 100 }));
    const f = setup({ status: "candidate", family: "PID", values: { "P1.Kp": 18, "P1.Ki": 12, "P1.Kd": 7, "P1.Kv": 1, "P1.Ka": 1 } }, rows);
    try {
      assert.deepStrictEqual(Array.from(f.root.querySelectorAll("[data-studio-manual-settings] label[for]")).map(label => label.textContent), labels);
    } finally { f.restore(); }
  });

  test("manual PI exposes the declared anti-windup gain without a Pin checkbox", () => {
    const rows = [...PID_ROWS, { symbol: "P1.TrackingGain", label: "Anti-windup tracking gain", unit: "1/s", minimum: 0, maximum: 10 }];
    const f = setup({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 2,
      "P1.Parameters.IntegralGain": 0.5, "P1.TrackingGain": 1 }, settingsPins: { values: { "P1.TrackingGain": 1 } } }, rows,
      family => family === "PI" ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain", "P1.TrackingGain"] : undefined);
    try {
      assert.ok(f.root.querySelector("[data-studio-manual-value='P1.TrackingGain']"), "PI requires its declared anti-windup setting");
      assert.strictEqual(f.controls.read().values["P1.TrackingGain"], "1", "the entry is read as typed");
      assert.strictEqual(f.root.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
    } finally { f.restore(); }
  });

  test("project-owned PID compositions expose all declared filter and reference settings", () => {
    const rows = [...PID_ROWS, { symbol: "P1.FilterTime", label: "Measurement filter time constant", unit: "s", minimum: 0, maximum: 10 }];
    const f = setup({ status: "candidate", family: "PI", values: { "P1.Parameters.ProportionalGain": 2,
      "P1.Parameters.IntegralGain": 0.5, "P1.Parameters.DerivativeGain": 0, "P1.FilterTime": 0.25 } }, rows);
    try {
      assert.ok(f.root.querySelector("[data-studio-manual-value='P1.FilterTime']"), "The project adapter owns the full composed controller");
      assert.strictEqual(f.controls.read().values["P1.FilterTime"], "0.25", "the entry is read as typed");
    } finally { f.restore(); }
  });

  test("shows editable adapter settings and explicit pin controls in the result area", () => {
    const dom = new JSDOM(`<main><div data-studio-tuning-result></div></main>`);
    const previousDocument = (globalThis as any).document;
    const previousWindow = (globalThis as any).window;
    (globalThis as any).document = dom.window.document;
    (globalThis as any).window = dom.window;
    try {
      const area = dom.window.document.querySelector("[data-studio-tuning-result]") as HTMLElement;
      const payload = {
      status: "candidate", family: "PID",
      values: {
        "P1.Parameters.ProportionalGain": 2,
        "P1.Parameters.IntegralGain": 0.5,
        "P1.Parameters.DerivativeGain": 0.1,
      },
      settingsPins: { values: { "P1.Parameters.IntegralGain": 0.5 } },
      words: { outcome: "Meets the requirements", settingLabels: [{ symbol: "P1.Parameters.ProportionalGain", label: "Proportional gain (Kp)" },
        { symbol: "P1.Parameters.IntegralGain", label: "Integral gain (Ki)" }, { symbol: "P1.Parameters.DerivativeGain", label: "Derivative gain (Kd)" }] },
      };
      renderTunedController(dom.window.document.querySelector("main") as HTMLElement, loop, payload);
      // An automatic result offers a Keep toggle per calculated setting; the pinned one starts kept.
      assert.strictEqual((area.querySelector("[data-studio-keep='P1.Parameters.IntegralGain']") as HTMLInputElement)?.checked, true);
      offerRows(PID_ROWS);
      initializeManualTuneSettings(dom.window.document.querySelector("main") as HTMLElement, undefined, () => PID_ROWS.map(row => row.symbol)).render(payload);
      assert.ok(area.querySelector("[data-studio-manual-settings]"), "Tune result needs a manual settings editor");
      assert.strictEqual(area.querySelectorAll("input[data-studio-manual-value]").length, 3);
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.IntegralGain']") as HTMLInputElement)?.value, "0.5");
      assert.strictEqual(area.querySelector("[data-studio-manual-pin]"), null, "manual settings have no Pin checkboxes");
      assert.match(area.textContent ?? "", /%\/°C/);
    } finally {
      (globalThis as any).document = previousDocument;
      (globalThis as any).window = previousWindow;
      setOpenedOffers({});
      dom.window.close();
    }
  });

  test("uses only the selected family, keeps full numeric precision, and avoids extra headings", () => {
    const candidate = { status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2.123456789,
      "P1.Parameters.IntegralGain": 0.500000001,
      "P1.Parameters.DerivativeGain": 0.1,
    } };
    const f = setup(candidate, PID_ROWS, family => family === "PI" ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"] : undefined);
    try {
      const area = f.root.querySelector("[data-studio-tuning-result]")!;
      assert.strictEqual(area.querySelectorAll("input[data-studio-manual-value]").length, 2);
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.ProportionalGain']") as HTMLInputElement).value, "2.123456789");
      assert.strictEqual((area.querySelector("[data-studio-manual-value='P1.Parameters.IntegralGain']") as HTMLInputElement).value, "0.500000001");
      assert.strictEqual(area.querySelector("legend"), null);
      assert.strictEqual(area.querySelector("small"), null);
    } finally { f.restore(); }
  });

  test("sends each entry as typed, so that an entry no number reads reaches the engine, which refuses it (B4.5)", () => {
    const f = setup({ status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5,
    } }, PID_ROWS, family => family === "PI" ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"] : undefined);
    try {
      const field = f.root.querySelector<HTMLInputElement>("[data-studio-manual-value='P1.Parameters.IntegralGain']")!;
      field.value = "";
      assert.deepStrictEqual(f.controls.read().values, { "P1.Parameters.ProportionalGain": "2", "P1.Parameters.IntegralGain": "" },
        "a blank entry is sent as blank: the page checks no range or finiteness");
      field.value = " 0.500000001 ";
      assert.deepStrictEqual(f.controls.read().values, { "P1.Parameters.ProportionalGain": "2", "P1.Parameters.IntegralGain": "0.500000001" },
        "an entry is sent as typed, without its surrounding spaces");
    } finally { f.restore(); }
  });

  test("marks only the row that the engine's refusal names, and sends an out-of-bounds entry as typed (B4.5)", () => {
    const f = setup({ status: "candidate", family: "PI", values: {
      "P1.Parameters.ProportionalGain": 2, "P1.Parameters.IntegralGain": 0.5,
    } }, PID_ROWS, family => family === "PI" ? ["P1.Parameters.ProportionalGain", "P1.Parameters.IntegralGain"] : undefined);
    try {
      const row = (symbol: string) => f.root.querySelector<HTMLInputElement>(`[data-studio-manual-value='${symbol}']`)!;
      row("P1.Parameters.ProportionalGain").value = "150";
      assert.strictEqual(f.controls.read().values["P1.Parameters.ProportionalGain"], "150", "the engine checks the bounds, not the page");
      f.controls.refuse("P1.Parameters.ProportionalGain");
      assert.strictEqual(row("P1.Parameters.ProportionalGain").getAttribute("aria-invalid"), "true", "the refused row is marked");
      assert.ok(row("P1.Parameters.ProportionalGain").classList.contains("trust-input--error"), "the refused row is outlined");
      assert.strictEqual(row("P1.Parameters.IntegralGain").getAttribute("aria-invalid"), null, "no other row is marked");
      assert.ok(!row("P1.Parameters.IntegralGain").classList.contains("trust-input--error"), "no other row is outlined");
      f.controls.refuse(undefined);
      assert.strictEqual(row("P1.Parameters.ProportionalGain").getAttribute("aria-invalid"), null, "a refusal that names no row clears the mark");
    } finally { f.restore(); }
  });

  test("a served program's manual form asks only for the symbols the engine names for the chosen family: family P on a PI seed asks for Kp alone (C2)", () => {
    const rows = [
      { symbol: "P1.Kp", label: "Proportional gain (Kp)", unit: "%/degC", minimum: 0, maximum: 20 },
      { symbol: "P1.Ki", label: "Integral gain (Ki)", unit: "%/(degC*s)", minimum: 0, maximum: 10 }];
    const named: Record<string, string[]> = { P: ["P1.Kp"], PI: ["P1.Kp", "P1.Ki"] };
    const f = setup({ status: "candidate", family: "P", values: { "P1.Kp": 2, "P1.Ki": 0.5 } }, rows, family => named[family]);
    try {
      const asked = () => Array.from(f.root.querySelectorAll<HTMLInputElement>("[data-studio-manual-value]"), input => input.dataset.studioManualValue);
      assert.deepStrictEqual(asked(), ["P1.Kp"], "family P asks for the proportional gain alone");
      assert.deepStrictEqual(f.controls.read(), { values: { "P1.Kp": "2" } }, "and reads only that setting, as typed");
      f.controls.render({ status: "candidate", family: "PI", values: { "P1.Kp": 2, "P1.Ki": 0.5 } });
      assert.deepStrictEqual(asked(), ["P1.Kp", "P1.Ki"], "family PI asks for both gains");
    } finally { f.restore(); }
  });
});
