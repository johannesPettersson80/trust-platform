import * as assert from "assert";

suite("Plant editor identity", () => {
  test("publishes one model-first option list containing every built-in model", () => {
    const descriptors = require("../../networkCanvas/plant/modelDescriptors") as {
      plantModelDescriptors?: () => Array<{ kind: string; productName: string }>;
    };
    assert.deepStrictEqual(descriptors.plantModelDescriptors?.().map(({ kind, productName }) => ({ kind, productName })), [
      { kind: "static_gain", productName: "Static gain" },
      { kind: "first_order", productName: "First order" },
      { kind: "first_order_dead_time", productName: "First order with dead time" },
      { kind: "integrator", productName: "Integrator" },
      { kind: "second_order", productName: "Second order" },
      { kind: "piecewise_linear", productName: "Piecewise linear" },
      { kind: "state_space", productName: "State space" },
      { kind: "gravity_tank", productName: "Gravity-drained tank" },
      { kind: "gravity_tank_cascade", productName: "Cascaded gravity tanks" },
      { kind: "robot_joint", productName: "Gravity-loaded robot joint" },
    ]);
  });

  test("edits the runtime gravity-tank model with physical parameter names and units", () => {
    const model = require("../../networkCanvas/plant/editorModel") as {
      plantDynamicsFields?: (kind: string) => Array<{ key: string; label: string; unit: string }>;
    };
    const descriptors = require("../../networkCanvas/plant/modelDescriptors") as {
      plantModelDescriptor?: (kind: string) => { productName: string; purpose: string; states: number };
    };

    assert.deepStrictEqual(model.plantDynamicsFields?.("gravity_tank"), [
      {
        key: "area_m2", label: "Tank area (m²)", valueKind: "number",
        plainTerm: "Tank cross-section", controlTerm: "Cross-sectional area", unit: "m²",
        effect: "A larger area makes the level change more slowly for the same flow.",
        storedKey: "parameters.area_m2", optional: false,
      },
      {
        key: "outflow_coefficient_m2_sqrt_m_per_s", label: "Outlet coefficient", valueKind: "number",
        plainTerm: "Outlet size", controlTerm: "Square-root discharge coefficient", unit: "m²√m/s",
        effect: "A larger coefficient drains the tank faster, especially at higher level.",
        storedKey: "parameters.outflow_coefficient_m2_sqrt_m_per_s", optional: false,
      },
      {
        key: "initial_state", label: "Initial level", valueKind: "vector",
        plainTerm: "Starting internal value", controlTerm: "Initial state vector", unit: "state units",
        effect: "This is the model state at simulation start or reset.",
        storedKey: "initial_state", optional: false,
      },
    ]);
    assert.deepStrictEqual(descriptors.plantModelDescriptor?.("gravity_tank"), {
      kind: "gravity_tank",
      productName: "Gravity-drained tank",
      purpose: "Inflow raises the level; gravity drains it faster as the level rises.",
      states: 1,
      inputs: 1,
      outputs: 1,
    });
  });

  test("edits the runtime nonlinear robot-joint model without a teaching-only path", () => {
    const model = require("../../networkCanvas/plant/editorModel") as {
      plantDynamicsFields?: (kind: string) => Array<{ key: string }>;
    };
    const descriptors = require("../../networkCanvas/plant/modelDescriptors") as {
      plantModelDescriptor?: (kind: string) => { productName: string; purpose: string; states: number; inputs: number; outputs: number };
    };
    assert.deepStrictEqual(
      model.plantDynamicsFields?.("robot_joint")?.map((field) => field.key),
      [
        "inertia_kg_m2",
        "gravity_torque_nm",
        "viscous_friction_nm_s_per_rad",
        "coulomb_friction_nm",
        "friction_smoothing_rad_s",
        "initial_state",
      ]
    );
    assert.deepStrictEqual(descriptors.plantModelDescriptor?.("robot_joint"), {
      kind: "robot_joint",
      productName: "Gravity-loaded robot joint",
      purpose: "Torque changes joint angle and speed; gravity depends on angle, and friction resists motion.",
      states: 2,
      inputs: 1,
      outputs: 2,
    });
  });

  test("describes, validates, and persists the scalar actuator settings", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      PLANT_ACTUATOR_FIELDS?: Array<{ key: string; label: string }>;
      validatePlantActuator?: (value: Record<string, number>) => Array<{ field: string; message: string }>;
      plantCandidateFromEditorDraft?: (draft: Record<string, unknown>) => Record<string, unknown>;
    };
    assert.deepStrictEqual(api.PLANT_ACTUATOR_FIELDS?.map((field) => field.key), [
      "minimum",
      "maximum",
      "rate_up_per_s",
      "rate_down_per_s",
      "deadband",
      "stiction_band",
      "slip_jump",
      "characteristic_exponent",
      "lag_s",
      "initial_applied",
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ minimum: 5, maximum: 4 }), [
      { field: "maximum", message: "Maximum must be greater than or equal to minimum." },
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ rate_up_per_s: -1 }), [
      { field: "rate_up_per_s", message: "Rising rate must be 0 or greater." },
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ stiction_band: 3, slip_jump: 4 }), [
      { field: "slip_jump", message: "Slip jump must not exceed the stiction band." },
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ characteristic_exponent: 0 }), [
      { field: "characteristic_exponent", message: "Installed characteristic exponent must be greater than 0." },
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ characteristic_exponent: 2 }), [
      { field: "characteristic_exponent", message: "A nonlinear installed characteristic requires finite minimum and maximum values with minimum below maximum." },
    ]);
    assert.deepStrictEqual(api.validatePlantActuator?.({ minimum: 0, maximum: 10, initial_applied: 11 }), [
      { field: "initial_applied", message: "Initial applied value must be within minimum and maximum." },
    ]);

    const base = {
      plantId: "tank",
      displayName: "Tank",
      description: "",
      enabled: true,
      model: "first_order",
      commandSources: ["%QW0"],
      feedbackTargets: ["%IW0"],
      parameters: { gain: 1, time_constant_s: 5 },
      initialState: [0],
    };
    assert.deepStrictEqual(api.plantCandidateFromEditorDraft?.({ ...base, actuator: {} })?.actuator, undefined);
    assert.deepStrictEqual(
      api.plantCandidateFromEditorDraft?.({ ...base, actuator: { minimum: 0, maximum: 100, characteristic_exponent: 2 } })?.actuator,
      { minimum: 0, maximum: 100, characteristic_exponent: 2 }
    );
  });

  test("publishes the runtime-resolved meaning for every blank actuator field", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      PLANT_ACTUATOR_FIELDS?: Array<{
        key: string;
        blankMeaning?: string;
        defaultValue?: number;
        defaultFrom?: string;
      }>;
    };
    assert.deepStrictEqual(api.PLANT_ACTUATOR_FIELDS?.map(({ key, blankMeaning, defaultValue, defaultFrom }) => ({
      key, blankMeaning, defaultValue, defaultFrom,
    })), [
      { key: "minimum", blankMeaning: "Unbounded", defaultValue: undefined, defaultFrom: undefined },
      { key: "maximum", blankMeaning: "Unbounded", defaultValue: undefined, defaultFrom: undefined },
      { key: "rate_up_per_s", blankMeaning: "Unbounded; 0 blocks upward movement", defaultValue: undefined, defaultFrom: undefined },
      { key: "rate_down_per_s", blankMeaning: "Unbounded; 0 blocks downward movement", defaultValue: undefined, defaultFrom: undefined },
      { key: "deadband", blankMeaning: "0 (no deadband)", defaultValue: 0, defaultFrom: undefined },
      { key: "stiction_band", blankMeaning: "0 (no static friction)", defaultValue: 0, defaultFrom: undefined },
      { key: "slip_jump", blankMeaning: "0 (no breakaway jump)", defaultValue: 0, defaultFrom: undefined },
      { key: "characteristic_exponent", blankMeaning: "1 (linear characteristic)", defaultValue: 1, defaultFrom: undefined },
      { key: "lag_s", blankMeaning: "0 (no lag)", defaultValue: 0, defaultFrom: undefined },
      { key: "initial_applied", blankMeaning: "Initial input (resolved at runtime)", defaultValue: undefined, defaultFrom: "initial_input" },
    ]);
  });

  test("accepts runtime-defaulted gain and FOPDT dead time when omitted", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      validatePlantDynamics?: (input: { model: string; parameters: Record<string, unknown>; initialState: number[] }) => Array<{ field: string }>;
    };
    assert.deepStrictEqual(api.validatePlantDynamics?.({
      model: "static_gain",
      parameters: {},
      initialState: [],
    }), []);
    assert.deepStrictEqual(api.validatePlantDynamics?.({
      model: "first_order_dead_time",
      parameters: { time_constant_s: 5 },
      initialState: [0],
    }), []);
    for (const [model, parameters, initialState] of [
      ["first_order", { time_constant_s: 5 }, [0]],
      ["integrator", {}, [0]],
      ["second_order", { natural_frequency_rad_s: 1, damping_ratio: 0.7 }, [0, 0]],
    ] as const) {
      assert.deepStrictEqual(api.validatePlantDynamics?.({ model, parameters, initialState: [...initialState] }), [], model);
    }
  });

  test("builds the exact persisted candidate from the visible editor draft", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      plantCandidateFromEditorDraft?: (draft: Record<string, unknown>) => unknown;
    };
    assert.deepStrictEqual(
      api.plantCandidateFromEditorDraft?.({
        plantId: "tank",
        displayName: "Training tank",
        description: "Slow level process",
        enabled: true,
        model: "first_order",
        commandSources: ["%QW0"],
        feedbackTargets: ["%IW0"],
        parameters: { gain: 2, time_constant_s: 5 },
        initialState: [1],
      }),
      {
        id: "tank",
        name: "Training tank",
        description: "Slow level process",
        enabled: true,
        model: "first_order",
        command_sources: ["%QW0"],
        feedback_targets: ["%IW0"],
        parameters: { gain: 2, time_constant_s: 5 },
        initial_state: [1],
        scenario_events: [],
      }
    );
  });

  test("persists the plant step and exact command and feedback engineering conversions", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      plantCandidateFromEditorDraft?: (draft: Record<string, unknown>) => Record<string, unknown>;
      validatePlantSignalEngineering?: (draft: Record<string, unknown>) => unknown[];
    };
    const draft = {
      plantId: "tank",
      displayName: "Tank",
      description: "",
      enabled: true,
      model: "first_order",
      commandSources: ["%QW0"],
      feedbackTargets: ["%IW0"],
      parameters: { gain: 1, time_constant_s: 5, input_bias: 0, output_bias: 0 },
      initialState: [0],
      stepMs: 5,
      initialInput: [2.5],
      inputScale: [0.1],
      inputOffset: [-1],
      outputScale: [10],
      outputOffset: [4],
    };
    assert.deepStrictEqual(api.validatePlantSignalEngineering?.(draft), []);
    assert.deepStrictEqual(api.plantCandidateFromEditorDraft?.(draft), {
      id: "tank",
      name: "Tank",
      description: "",
      enabled: true,
      model: "first_order",
      command_sources: ["%QW0"],
      feedback_targets: ["%IW0"],
      step_ms: 5,
      initial_input: [2.5],
      input_scale: [0.1],
      input_offset: [-1],
      output_scale: [10],
      output_offset: [4],
      parameters: { gain: 1, time_constant_s: 5, input_bias: 0, output_bias: 0 },
      initial_state: [0],
      scenario_events: [],
    });
    assert.deepStrictEqual(api.validatePlantSignalEngineering?.({
      ...draft,
      stepMs: 0,
      inputScale: [1, 2],
      outputOffset: [Number.NaN],
    }), [
      { field: "step_ms", message: "Plant step must be a positive whole number of milliseconds." },
      { field: "input_scale", message: "Input scale needs exactly 1 value for the 1 command signal." },
      { field: "output_offset", message: "Output offset must contain only finite numbers." },
    ]);
  });

  test("creates a unique editable identity without changing runtime placement", () => {
    let api:
      | {
          createPlantEditorDraft?: (input: {
            runtimeId: string;
            runtimeName: string;
            existingPlantIds: string[];
          }) => unknown;
        }
      | undefined;
    try {
      api = require("../../networkCanvas/plant/editorModel") as typeof api;
    } catch {
      api = undefined;
    }

    assert.deepStrictEqual(
      api?.createPlantEditorDraft?.({
        runtimeId: "runtime:local",
        runtimeName: "Simulator",
        existingPlantIds: ["tank_level", "new_plant"],
      }),
      {
        plantId: "new_plant_2",
        displayName: "New simulated plant",
        description: "",
        runtimeId: "runtime:local",
        runtimeName: "Simulator",
        enabled: true,
        startingPoint: "temperature",
        model: "first_order_dead_time",
        parameters: {
          gain: 1,
          time_constant_s: 10,
          dead_time_s: 1,
          input_bias: 0,
          output_bias: 0,
          initial_delayed_input: 0,
        },
        initialState: [0],
        modelValuesTouched: false,
        commandSources: [],
        feedbackTargets: [],
        initialInput: [],
        inputScale: [],
        inputOffset: [],
        outputScale: [],
        outputOffset: [],
        scenarioEvents: [],
        dirty: true,
      }
    );
  });

  test("reports exact corrective identity and display-name errors", () => {
    let api:
      | {
          validatePlantIdentity?: (input: {
            id: string;
            displayName: string;
            existingPlantIds: string[];
            originalId?: string;
          }) => unknown;
        }
      | undefined;
    try {
      api = require("../../networkCanvas/plant/editorModel") as typeof api;
    } catch {
      api = undefined;
    }

    assert.deepStrictEqual(
      api?.validatePlantIdentity?.({
        id: "1 tank",
        displayName: " ",
        existingPlantIds: ["tank_level"],
      }),
      [
        {
          field: "id",
          code: "invalid_plant_id",
          message: "Start with a letter; then use letters, numbers, _ or -.",
        },
        {
          field: "displayName",
          code: "empty_display_name",
          message: "Enter a display name.",
        },
      ]
    );
    assert.deepStrictEqual(
      api?.validatePlantIdentity?.({
        id: "tank_level",
        displayName: "Tank",
        existingPlantIds: ["tank_level"],
      }),
      [
        {
          field: "id",
          code: "duplicate_plant_id",
          message: "Choose an ID that is not already used in this project.",
        },
      ]
    );
  });
});

suite("Plant editor starting points", () => {
  test("defines the eight common starting points and their exact editable proposals", () => {
    let api:
      | {
          PLANT_STARTING_POINTS?: ReadonlyArray<unknown>;
          proposePlantStartingPoint?: (id: string) => unknown;
        }
      | undefined;
    try {
      api = require("../../networkCanvas/plant/editorModel") as typeof api;
    } catch {
      api = undefined;
    }

    assert.deepStrictEqual(
      api?.PLANT_STARTING_POINTS?.map((entry: any) => ({
        id: entry.id,
        label: entry.label,
        model: entry.model,
      })),
      [
        { id: "tank", label: "Tank or accumulated level", model: "integrator" },
        { id: "gravity_drained_tank", label: "Gravity-drained tank", model: "gravity_tank" },
        { id: "gravity_tank_cascade", label: "Cascaded gravity tanks", model: "gravity_tank_cascade" },
        { id: "temperature", label: "Temperature or slow process", model: "first_order_dead_time" },
        { id: "motor_speed", label: "Motor speed or first-order process", model: "first_order" },
        { id: "mechanical_position", label: "Mechanical position or resonant process", model: "second_order" },
        { id: "robot_joint", label: "Gravity-loaded robot joint", model: "robot_joint" },
        { id: "static_scaling", label: "Static scaling", model: "static_gain" },
        { id: "nonlinear_characteristic", label: "Nonlinear characteristic", model: "piecewise_linear" },
      ]
    );

    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("tank"), {
      model: "integrator",
      parameters: {
        gain: 1,
        input_bias: 0,
        output_bias: 0,
      },
      initialState: [0],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("gravity_drained_tank"), {
      model: "gravity_tank",
      parameters: {
        area_m2: 2,
        outflow_coefficient_m2_sqrt_m_per_s: 0.5,
      },
      initialState: [4],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("robot_joint"), {
      model: "robot_joint",
      parameters: {
        inertia_kg_m2: 2,
        gravity_torque_nm: 4,
        viscous_friction_nm_s_per_rad: 0.2,
        coulomb_friction_nm: 0.1,
        friction_smoothing_rad_s: 0.01,
      },
      initialState: [0.5, 0],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("temperature"), {
      model: "first_order_dead_time",
      parameters: {
        gain: 1,
        time_constant_s: 10,
        dead_time_s: 1,
        input_bias: 0,
        output_bias: 0,
        initial_delayed_input: 0,
      },
      initialState: [0],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("motor_speed"), {
      model: "first_order",
      parameters: {
        gain: 1,
        time_constant_s: 1,
        input_bias: 0,
        output_bias: 0,
      },
      initialState: [0],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("mechanical_position"), {
      model: "second_order",
      parameters: {
        gain: 1,
        natural_frequency_rad_s: 1,
        damping_ratio: 0.7,
        input_bias: 0,
        output_bias: 0,
      },
      initialState: [0, 0],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("static_scaling"), {
      model: "static_gain",
      parameters: { gain: 1, input_bias: 0, output_bias: 0 },
      initialState: [],
    });
    assert.deepStrictEqual(api?.proposePlantStartingPoint?.("nonlinear_characteristic"), {
      model: "piecewise_linear",
      parameters: { input_points: [-1, 0, 1], output_points: [-1, 0, 1] },
      initialState: [],
    });
  });

  test("proposes a deterministic advanced one-state state-space model", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      proposeCustomStateSpace?: () => unknown;
    };

    assert.deepStrictEqual(api.proposeCustomStateSpace?.(), {
      startingPoint: "custom_state_space",
      model: "state_space",
      parameters: {
        a: [[0]],
        b: [[1]],
        c: [[1]],
        d: [[0]],
      },
      initialState: [0],
    });
  });

  test("provides specific reasoning and limitations for every starting point", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      PLANT_STARTING_POINTS: Array<{ id: string; whyReasonable?: string; limitation?: string }>;
      CUSTOM_STATE_SPACE_HELP?: { whyReasonable: string; limitation: string };
    };
    const help: Record<string, [string | undefined, string | undefined] | undefined> = {};
    for (const entry of api.PLANT_STARTING_POINTS) {
      help[entry.id] = [entry.whyReasonable, entry.limitation];
    }
    help.custom_state_space = api.CUSTOM_STATE_SPACE_HELP && [
      api.CUSTOM_STATE_SPACE_HELP.whyReasonable,
      api.CUSTOM_STATE_SPACE_HELP.limitation,
    ];

    assert.deepStrictEqual(Object.keys(help), [
      "tank",
      "gravity_drained_tank",
      "gravity_tank_cascade",
      "temperature",
      "motor_speed",
      "mechanical_position",
      "robot_joint",
      "static_scaling",
      "nonlinear_characteristic",
      "custom_state_space",
    ]);
    for (const [id, copy] of Object.entries(help)) {
      assert.ok(copy?.[0]?.length, `${id} must explain why the model is reasonable`);
      assert.match(copy?.[1] ?? "", /cannot|without|omits|assumes no/i,
        `${id} must state what it cannot reproduce`);
    }
    assert.match(help.temperature?.[0] ?? "", /lag.*delay/i);
    assert.match(help.mechanical_position?.[0] ?? "", /inertia.*damping/i);
    assert.match(help.nonlinear_characteristic?.[1] ?? "", /hysteresis/i);
  });

  test("requires confirmation before replacing user-edited model values", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      planPlantStartingPointChange?: (input: {
        currentStartingPoint: string;
        requestedStartingPoint: string;
        modelValuesTouched: boolean;
      }) => unknown;
    };
    assert.deepStrictEqual(
      api.planPlantStartingPointChange?.({
        currentStartingPoint: "temperature",
        requestedStartingPoint: "motor_speed",
        modelValuesTouched: false,
      }),
      { kind: "apply", requestedStartingPoint: "motor_speed" }
    );
    assert.deepStrictEqual(
      api.planPlantStartingPointChange?.({
        currentStartingPoint: "temperature",
        requestedStartingPoint: "motor_speed",
        modelValuesTouched: true,
      }),
      {
        kind: "confirm",
        currentStartingPoint: "temperature",
        requestedStartingPoint: "motor_speed",
        message:
          "Replace your edited Temperature or slow process dynamics with the Motor speed or first-order process starting values?",
      }
    );
  });
});

suite("Plant imperfection editor", () => {
  test("configures backlash as a process-output play effect with positive width", () => {
    const api = require("../../networkCanvas/plant/uncertaintyEditor") as any;
    const valid = {
      ...api.createPlantUncertainty(1),
      id: "gear-clearance",
      target: "process_output",
      mode: "replace",
      generator: "backlash",
      width: 0.05,
    };

    assert.deepStrictEqual(api.plantUncertaintyGeneratorFields("backlash"), ["width"]);
    assert.strictEqual(api.plantUncertaintyFieldLabel("width"), "Backlash width");
    assert.deepStrictEqual(api.validatePlantUncertainties([valid], []), []);
    assert.deepStrictEqual(api.persistedPlantUncertainties([valid]), [{
      id: "gear-clearance",
      target: "process_output",
      mode: "replace",
      generator: "backlash",
      channel: 0,
      start_s: 0,
      width: 0.05,
    }]);
    assert.deepStrictEqual(api.validatePlantUncertainties([
      { ...valid, target: "measurement" },
      { ...valid, id: "zero-clearance", width: 0 },
    ], []), [
      { row: 0, field: "generator", message: "Backlash requires Process output and Replace mode." },
      { row: 1, field: "width", message: "Backlash width must be greater than 0." },
    ]);
  });

  test("owns valid generator fields and preserves explicit row order in the saved candidate", () => {
    let api: any;
    try { api = require("../../networkCanvas/plant/uncertaintyEditor"); } catch { api = undefined; }
    assert.deepStrictEqual(api?.plantUncertaintyGeneratorFields?.("normal_approx_white"), ["mean", "standard_deviation"]);
    assert.deepStrictEqual(api?.plantUncertaintyGeneratorFields?.("dropout"), [
      "rate_per_s", "duration_s", "dropout_policy", "replacement_value",
    ]);
    const rows = [
      api?.createPlantUncertainty?.(1),
      { ...api?.createPlantUncertainty?.(2), id: "noise", generator: "normal_approx_white", standard_deviation: 0.1 },
    ];
    assert.deepStrictEqual(api?.movePlantUncertainty?.(rows, 1, -1)?.map((row: any) => row.id), ["noise", "effect_1"]);
    assert.deepStrictEqual(api?.validatePlantUncertainties?.([
      { ...api?.createPlantUncertainty?.(1), id: "bad id" },
      { ...api?.createPlantUncertainty?.(2), id: "effect_2", generator: "dropout", target: "measurement", mode: "replace", rate_per_s: 1, duration_s: 0, dropout_policy: "hold_last" },
    ], ["gain"]), [
      { row: 0, field: "id", message: "Start with a letter; then use letters, numbers, _ or -." },
      { row: 1, field: "duration_s", message: "Duration must be greater than 0 s." },
    ]);
    assert.deepStrictEqual(api?.persistedPlantUncertainties?.([
      { ...api?.createPlantUncertainty?.(1), id: "bias", value: 2 },
      { ...api?.createPlantUncertainty?.(2), id: "disabled", enabled: false },
    ]), [
      { id: "bias", target: "measurement", mode: "add", generator: "constant", channel: 0, start_s: 0, value: 2 },
    ]);
  });

  test("maps concise effect types to exact runtime uncertainty contracts", () => {
    let api: any;
    try { api = require("../../networkCanvas/plant/imperfectionPresets"); } catch { api = undefined; }
    assert.deepStrictEqual(api?.PLANT_IMPERFECTION_TYPES?.map((entry: any) => entry.id), [
      "bias", "drift", "gaussian_noise", "quantization", "sample_and_hold",
      "dropout", "stuck_sensor", "outlier", "custom",
    ]);
    assert.deepStrictEqual(api?.applyPlantImperfectionType?.("gaussian_noise", { id: "noise", enabled: true }), {
      id: "noise", enabled: true, target: "measurement", channel: 0, mode: "add",
      generator: "normal_approx_white", mean: 0, standard_deviation: 0.01, start_s: 0,
    });
    assert.deepStrictEqual(api?.applyPlantImperfectionType?.("quantization", { id: "quant", enabled: true }), {
      id: "quant", enabled: true, target: "measurement", channel: 0, mode: "replace",
      generator: "quantise", quantum: 0.01, start_s: 0,
    });
    assert.deepStrictEqual(api?.applyPlantImperfectionType?.("stuck_sensor", { id: "stuck", enabled: true }), {
      id: "stuck", enabled: true, target: "measurement", channel: 0, mode: "hold",
      generator: "constant", start_s: 0,
    });
    assert.strictEqual(api?.plantImperfectionTypeForRow?.({
      id: "noise", enabled: true, target: "measurement", channel: 0, mode: "add",
      generator: "normal_approx_white", mean: 0, standard_deviation: 0.2, start_s: 0,
    }), "gaussian_noise");
  });

  test("accepts every concise imperfection preset without hidden required fields", () => {
    const presets = require("../../networkCanvas/plant/imperfectionPresets") as {
      applyPlantImperfectionType: (type: string, current: { id: string; enabled: boolean }) => unknown;
    };
    const editor = require("../../networkCanvas/plant/uncertaintyEditor") as {
      validatePlantUncertainties: (rows: unknown[], modelParameters: string[]) => unknown[];
      persistedPlantUncertainties: (rows: unknown[]) => Array<Record<string, unknown>>;
    };
    const types = [
      "bias", "drift", "gaussian_noise", "quantization", "sample_and_hold",
      "dropout", "stuck_sensor", "outlier",
    ];
    const rows = types.map((type) =>
      presets.applyPlantImperfectionType(type, { id: type, enabled: true }) as Record<string, unknown>
    );
    assert.deepStrictEqual(rows.map(({ id, mode, generator }) => ({ id, mode, generator })), [
      { id: "bias", mode: "add", generator: "constant" },
      { id: "drift", mode: "add", generator: "random_walk" },
      { id: "gaussian_noise", mode: "add", generator: "normal_approx_white" },
      { id: "quantization", mode: "replace", generator: "quantise" },
      { id: "sample_and_hold", mode: "hold", generator: "constant" },
      { id: "dropout", mode: "replace", generator: "dropout" },
      { id: "stuck_sensor", mode: "hold", generator: "constant" },
      { id: "outlier", mode: "add", generator: "pulse" },
    ]);
    for (const [index, type] of types.entries()) {
      assert.deepStrictEqual(
        editor.validatePlantUncertainties([rows[index]], ["gain"]),
        [],
        `${type} must be immediately valid with its visible defaults`
      );
    }
    assert.deepStrictEqual(
      editor.persistedPlantUncertainties(rows).map((row) => row.id),
      types,
      "the displayed combination order must be the runtime execution order"
    );
  });
});

suite("Plant editor dynamics", () => {
  test("model selection uses the existing replacement confirmation decision", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      planPlantStartingPointChange: (input: {
        currentStartingPoint: string;
        requestedStartingPoint: string;
        modelValuesTouched: boolean;
      }) => { kind: string };
    };
    assert.strictEqual(api.planPlantStartingPointChange({
      currentStartingPoint: "temperature",
      requestedStartingPoint: "tank",
      modelValuesTouched: true,
    }).kind, "confirm");
    assert.strictEqual(api.planPlantStartingPointChange({
      currentStartingPoint: "temperature",
      requestedStartingPoint: "tank",
      modelValuesTouched: false,
    }).kind, "apply");
  });

  test("runtime-defaultable fields describe scalar and input-bias defaults", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      plantDynamicsFields?: (model: string) => Array<{ key: string; defaultValue?: number; defaultFrom?: string }>;
    };
    const fields = api.plantDynamicsFields?.("first_order_dead_time") ?? [];
    assert.deepStrictEqual(
      fields.filter((field) => ["input_bias", "output_bias", "initial_delayed_input"].includes(field.key))
        .map(({ key, defaultValue, defaultFrom }) => ({ key, defaultValue, defaultFrom })),
      [
        { key: "input_bias", defaultValue: 0, defaultFrom: undefined },
        { key: "output_bias", defaultValue: 0, defaultFrom: undefined },
        { key: "initial_delayed_input", defaultValue: undefined, defaultFrom: "input_bias" },
      ]
    );
  });

  test("provides compact, field-specific engineering help without changing stored values", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      plantDynamicsFields?: (model: string) => Array<{
        key: string;
        plainTerm: string;
        controlTerm: string;
        unit: string;
        effect: string;
        storedKey: string;
      }>;
    };

    const fields = api.plantDynamicsFields?.("first_order_dead_time") ?? [];
    assert.deepStrictEqual(
      fields.map(({ key, storedKey }) => ({ key, storedKey })),
      [
        { key: "gain", storedKey: "parameters.gain" },
        { key: "time_constant_s", storedKey: "parameters.time_constant_s" },
        { key: "dead_time_s", storedKey: "parameters.dead_time_s" },
        { key: "input_bias", storedKey: "parameters.input_bias" },
        { key: "output_bias", storedKey: "parameters.output_bias" },
        { key: "initial_delayed_input", storedKey: "parameters.initial_delayed_input" },
        { key: "initial_state", storedKey: "initial_state" },
      ]
    );
    for (const field of fields) {
      assert.ok(field.plainTerm.length > 0, `${field.key} needs a plain term`);
      assert.ok(field.controlTerm.length > 0, `${field.key} needs a control term`);
      assert.ok(field.unit.length > 0, `${field.key} needs a unit`);
      assert.ok(field.effect.length > 0, `${field.key} needs an effect explanation`);
    }
  });

  test("describes and validates every model through one shared parameter editor", () => {
    const api = require("../../networkCanvas/plant/editorModel") as {
      plantDynamicsFields?: (model: string) => Array<{
        key: string;
        label: string;
        valueKind: string;
      }>;
      validatePlantDynamics?: (input: {
        model: string;
        parameters: Record<string, number | number[] | number[][]>;
        initialState: number[];
      }) => Array<{ field: string; message: string }>;
    };

    const expected: Record<string, string[]> = {
      static_gain: ["gain", "input_bias", "output_bias"],
      first_order: ["gain", "time_constant_s", "input_bias", "output_bias", "initial_state"],
      first_order_dead_time: [
        "gain",
        "time_constant_s",
        "dead_time_s",
        "input_bias",
        "output_bias",
        "initial_delayed_input",
        "initial_state",
      ],
      integrator: [
        "gain",
        "input_bias",
        "output_bias",
        "state_minimum",
        "state_maximum",
        "initial_state",
      ],
      second_order: [
        "gain",
        "natural_frequency_rad_s",
        "damping_ratio",
        "input_bias",
        "output_bias",
        "initial_state",
      ],
      piecewise_linear: ["input_points", "output_points"],
      state_space: ["a", "b", "c", "d", "input_delays_s", "initial_state"],
    };

    for (const [model, keys] of Object.entries(expected)) {
      assert.deepStrictEqual(
        api.plantDynamicsFields?.(model).map((field) => field.key),
        keys,
        model
      );
    }

    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "first_order_dead_time",
        parameters: {
          gain: 1,
          time_constant_s: 0,
          dead_time_s: -1,
          input_bias: 0,
          output_bias: 0,
          initial_delayed_input: 0,
        },
        initialState: [0],
      }),
      [
        { field: "time_constant_s", message: "Time constant must be greater than 0 s." },
        { field: "dead_time_s", message: "Dead time must be 0 s or greater." },
      ]
    );

    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "piecewise_linear",
        parameters: { input_points: [0, 0], output_points: [0] },
        initialState: [],
      }),
      [
        { field: "output_points", message: "Input and output points must contain the same number of values." },
        { field: "input_points.1", message: "Input point 2 must be greater than input point 1." },
      ]
    );

    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "integrator",
        parameters: { gain: 2, input_bias: 1, output_bias: 3 },
        initialState: [4],
      }),
      [],
      "omitted integrator state limits are unbounded"
    );
    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "integrator",
        parameters: {
          gain: 2,
          input_bias: 1,
          output_bias: 3,
          state_minimum: 0,
          state_maximum: 10,
        },
        initialState: [4],
      }),
      []
    );
    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "first_order_dead_time",
        parameters: { gain: 1, time_constant_s: 10, dead_time_s: 1 },
        initialState: [0],
      }),
      [],
      "runtime-defaultable first-order values may be omitted"
    );
    assert.deepStrictEqual(
      api.validatePlantDynamics?.({
        model: "integrator",
        parameters: {
          gain: 2,
          input_bias: 1,
          output_bias: 3,
          state_minimum: 5,
          state_maximum: 4,
        },
        initialState: [4],
      }),
      [{ field: "state_maximum", message: "Maximum state must be greater than or equal to minimum state." }]
    );
  });

  test("round-trips every scalar model without changing its exact parameters", () => {
    const editor = require("../../networkCanvas/plant/editorModel") as {
      plantCandidateFromEditorDraft: (draft: Record<string, unknown>) => Record<string, unknown>;
    };
    const flow = require("../../networkCanvas/plant/addFlow") as {
      createPlantEditDraft: (input: Record<string, unknown>) => Record<string, unknown>;
    };
    const models = [
      { model: "static_gain", parameters: { gain: 2, input_bias: 1, output_bias: 3 }, initialState: [] },
      { model: "first_order", parameters: { gain: 2, time_constant_s: 4, input_bias: 1, output_bias: 3 }, initialState: [5] },
      { model: "integrator", parameters: { gain: 2, input_bias: 1, output_bias: 3, state_minimum: 0, state_maximum: 10 }, initialState: [5] },
      { model: "second_order", parameters: { gain: 2, natural_frequency_rad_s: 4, damping_ratio: 0.6, input_bias: 1, output_bias: 3 }, initialState: [5, 6] },
    ];

    for (const entry of models) {
      const common = {
        plantId: "process",
        displayName: "Process",
        description: "",
        enabled: true,
        model: entry.model,
        commandSources: ["%QW0"],
        feedbackTargets: ["%IW0"],
        parameters: entry.parameters,
        initialState: entry.initialState,
      };
      const saved = editor.plantCandidateFromEditorDraft(common);
      const reopened = flow.createPlantEditDraft({
        id: "plant:runtime:local:process",
        projectId: "project",
        runtimeId: "runtime:local",
        runtimeName: "Simulator",
        plantId: saved.id,
        displayName: saved.name,
        description: saved.description,
        enabled: saved.enabled,
        model: saved.model,
        parameters: saved.parameters,
        initialState: saved.initial_state,
        commandSources: saved.command_sources,
        feedbackTargets: saved.feedback_targets,
        savedConfigurationRevision: "saved",
      });
      assert.deepStrictEqual(reopened.parameters, entry.parameters, entry.model);
      assert.deepStrictEqual(reopened.initialState, entry.initialState, entry.model);
    }
  });
});

suite("Plant piecewise-linear editor", () => {
  test("edits paired points while preserving strict input order and exact serialization", () => {
    let api: any;
    try { api = require("../../networkCanvas/plant/piecewiseEditor"); } catch { api = undefined; }

    const points = api?.piecewisePointsFromParameters?.({
      input_points: [-1, 0, 1],
      output_points: [-2, 0, 3],
    });
    assert.deepStrictEqual(points, [
      { input: -1, output: -2 },
      { input: 0, output: 0 },
      { input: 1, output: 3 },
    ]);
    assert.deepStrictEqual(api?.appendPiecewisePoint?.(points), [
      ...points,
      { input: 2, output: 3 },
    ]);
    assert.deepStrictEqual(api?.validatePiecewisePoints?.([
      { input: -1, output: 0 },
      { input: -1, output: 1 },
    ]), [
      { row: 1, field: "input", message: "Input point 2 must be greater than input point 1." },
    ]);
    assert.deepStrictEqual(api?.movePiecewisePoint?.([
      { input: 1, output: 10 },
      { input: 0, output: 0 },
      { input: 2, output: 20 },
    ], 1, -1), {
      moved: true,
      points: [
        { input: 0, output: 0 },
        { input: 1, output: 10 },
        { input: 2, output: 20 },
      ],
    });
    assert.deepStrictEqual(api?.movePiecewisePoint?.(points, 1, -1), {
      moved: false,
      points,
    });
    assert.deepStrictEqual(api?.piecewiseParametersFromPoints?.(points), {
      input_points: [-1, 0, 1],
      output_points: [-2, 0, 3],
    });
  });
});
