/**
 * Control-loop fixtures for the W4 served-loop tests.
 *
 * Each export returns a complete loop object in the form the Rust engine reads (schemaVersion 2). The values are copies
 * of the Rust test fixtures named above each function. Where a Rust test builds its loop in code rather than JSON, the
 * function names the generator it follows, and its values are the ones that generator writes for the test's catalog and
 * draft. Every call returns a fresh object, so a test may change its copy.
 *
 * Revisions are placeholders with the sha256: prefix. The Rust generators write content digests, which these fixtures
 * do not reproduce.
 */

// ---------------------------------------------------------------------------
// Cascade (C6): the two-PID vessel of lesson 52, as one loop.
// ---------------------------------------------------------------------------

/** The inner PI's seven retained states on channel 0, as pid_states in setup/pid_bindings.rs writes them for cascade_loop.rs. */
function cascadeInnerStates(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Inner.StateValue.Setpoint", iec_type: "LREAL", unit: "degC", minimum: 0, maximum: 90,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.SetpointRate", iec_type: "LREAL", unit: "degC/s", minimum: -90000, maximum: 90000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.ProcessValue", iec_type: "LREAL", unit: "degC", minimum: 0, maximum: 90,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.ProcessValueRate", iec_type: "LREAL", unit: "degC/s", minimum: -90000, maximum: 90000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.FeedForward", iec_type: "LREAL", unit: "%", minimum: -100, maximum: 100,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.NominalOutput", iec_type: "LREAL", unit: "%", minimum: -1000, maximum: 1100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Inner.StateValue.SaturatedOutput", iec_type: "LREAL", unit: "%", minimum: 0, maximum: 100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
  ];
}

/** The outer PI's seven states on channel 1; its output starts at the inner measurement (outer_start in cascade_loop.rs). */
function cascadeOuterStates(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Outer.StateValue.Setpoint", iec_type: "LREAL", unit: "degC", minimum: 0, maximum: 90,
      initial: { source: "model-output", index: 1 }, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.SetpointRate", iec_type: "LREAL", unit: "degC/s", minimum: -90000, maximum: 90000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.ProcessValue", iec_type: "LREAL", unit: "degC", minimum: 0, maximum: 90,
      initial: { source: "model-output", index: 1 }, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.ProcessValueRate", iec_type: "LREAL", unit: "degC/s", minimum: -90000, maximum: 90000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.FeedForward", iec_type: "LREAL", unit: "degC", minimum: -90, maximum: 90,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.NominalOutput", iec_type: "LREAL", unit: "degC", minimum: -900, maximum: 990,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Outer.StateValue.SaturatedOutput", iec_type: "LREAL", unit: "degC", minimum: 0, maximum: 90,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
  ];
}

/** The cascade's program block: the seed of cascade_seed in project_cascade_tests.rs, with the generator's states and modes. */
function cascadeProject(): Record<string, unknown> {
  return {
    family: "cascade",
    convention: "project-owned-native-controller",
    initialization: {
      kind: "declared-state-v1",
      states: [...cascadeInnerStates(), ...cascadeOuterStates()],
    },
    seed: {
      kind: "cascade-pid",
      family: "PI",
      inner: {
        instance: "P1.Inner",
        proportionalSymbol: "P1.InnerParameters.ProportionalGain",
        integralSymbol: "P1.InnerParameters.IntegralGain",
        derivativeSymbol: "P1.InnerParameters.DerivativeGain",
      },
      outer: {
        instance: "P1.Outer",
        proportionalSymbol: "P1.OuterParameters.ProportionalGain",
        integralSymbol: "P1.OuterParameters.IntegralGain",
        derivativeSymbol: "P1.OuterParameters.DerivativeGain",
      },
    },
    additionalModes: [{ symbol: "P1.OuterAuto", manualValue: false }],
    axes: [],
    objective: { requirementId: "P1.VesselC:target-error", direction: "minimize" },
    maximumCandidates: 1,
    maximumCycles: 50000,
    wallBudgetMs: 60000,
  };
}

/** The six gains of both PI records, the inner's first (cascade_parameters in cascade_loop.rs). */
function cascadeParameters(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.InnerParameters.ProportionalGain", iec_type: "LREAL", unit: "%/degC",
      convention: "Control.PID ProportionalGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.InnerParameters.IntegralGain", iec_type: "LREAL", unit: "%/(degC*s)",
      convention: "Control.PID IntegralGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.InnerParameters.DerivativeGain", iec_type: "LREAL", unit: "%*s/degC",
      convention: "Control.PID DerivativeGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.OuterParameters.ProportionalGain", iec_type: "LREAL", unit: "degC/degC",
      convention: "Control.PID ProportionalGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.OuterParameters.IntegralGain", iec_type: "LREAL", unit: "degC/(degC*s)",
      convention: "Control.PID IntegralGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.OuterParameters.DerivativeGain", iec_type: "LREAL", unit: "degC*s/degC",
      convention: "Control.PID DerivativeGain", minimum: -1000000, maximum: 1000000,
    },
  ];
}

/** The loop's signals: the outer setpoint, measurement and the plant input, and the inner measurement as a further state signal. */
function cascadeSignals(): Record<string, unknown> {
  return {
    setpoint: {
      symbol: "P1.VesselSetpointC", address: "%ML0", iecType: "LREAL", direction: "read",
      unit: "degC", range: { minimum: 0, maximum: 90 },
    },
    processVariable: {
      symbol: "P1.VesselC", address: "%IL0", iecType: "LREAL", direction: "read",
      unit: "degC", range: { minimum: 0, maximum: 90 },
    },
    manipulatedVariable: {
      symbol: "P1.HeaterPct", address: "%QL0", iecType: "LREAL", direction: "write",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
    additional: [
      {
        id: "inner-measurement", channel: "inner-measurement", role: "state",
        symbol: "P1.JacketC", address: "%IL8", iecType: "LREAL", direction: "read",
        unit: "degC", range: { minimum: 0, maximum: 90 },
      },
    ],
  };
}

/** One plant input, and two outputs: the inner measurement without a setpoint, the outer measurement with the setpoint (channels of cascade_loop.rs). */
function cascadeChannels(): Record<string, unknown> {
  return {
    inputs: [
      {
        modelChannelId: "P1.HeaterPct",
        signalId: "manipulatedVariable",
        actuator: { kind: "continuous", minimum: 0, maximum: 100 },
        experimentStep: 10,
      },
    ],
    outputs: [
      { modelChannelId: "P1.JacketC", signalId: "inner-measurement" },
      { modelChannelId: "P1.VesselC", signalId: "processVariable", setpointSignalId: "setpoint" },
    ],
  };
}

/**
 * The cascade of lesson 52's vessel, one loop (C6).
 *
 * Source: the generator crates/trust-control/src/setup/cascade_loop.rs (cascade_loop_definition, lines 117-191) on the
 * vessel catalog and draft of crates/trust-control/src/setup/cascade_loop_tests.rs (vessel() lines 61-100, draft()
 * lines 102-137). The Rust test the_written_loop_reads_back_as_the_same_loop (lines 262-267) decodes the generated
 * loop. The program block's seed is cascade_seed in crates/trust-control/src/loop_definition/project_cascade_tests.rs
 * (lines 18-36). The channels are those of cascade_loop_tests.rs lines 195-212 and cascade_loop.rs lines 405-447.
 * The experiment is the draft's step of 10 for 120 s.
 */
export function cascadeLoop(): Record<string, unknown> {
  return {
    schemaVersion: 2,
    id: "loop:vessel-temperature",
    name: "Vessel temperature",
    revision: "sha256:cascade-vessel",
    projectId: "project:setup",
    runtimeId: "runtime-1",
    controller: {
      instance: "P1.Inner",
      type: "Control.PID",
      revision: "sha256:unverified",
      action: "direct",
      taskPeriodMs: 100,
      design: "project",
      modeSignal: "P1.InnerAuto",
      modeWhenTrue: "Auto",
      manualOutputSignal: "P1.InnerManual",
      manualOutputAddress: "%ML24",
      manualOutputType: "LREAL",
      project: cascadeProject(),
    },
    parameterBinding: {
      loop_id: "loop:vessel-temperature",
      task_name: "MainTask",
      parameters: cascadeParameters(),
      mode: { symbol: "P1.InnerAuto", manual_value: false },
    },
    signals: cascadeSignals(),
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    topology: "closed-loop",
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 120, seed: 1 },
    identificationPolicy: {
      maximumOrder: 1,
      openLoopPermitted: true,
      sustainedHoldPermitted: true,
      information: "basic",
    },
    channels: cascadeChannels(),
  };
}

// ---------------------------------------------------------------------------
// Smith predictor (C9): the tank of complete_loop, with the Smith law as its program's own law.
// ---------------------------------------------------------------------------

/** The signals of complete_loop in crates/trust-control/src/loop_definition/tests.rs (lines 47-52). */
function tankSignals(): Record<string, unknown> {
  return {
    setpoint: {
      symbol: "LevelSP", address: "%ML0", iecType: "LREAL", direction: "read",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
    processVariable: {
      symbol: "LevelPV", address: "%IL0", iecType: "LREAL", direction: "read",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
    manipulatedVariable: {
      symbol: "ValveMV", address: "%QL0", iecType: "LREAL", direction: "write",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
    actuator: {
      symbol: "ValvePosition", address: "%IL8", iecType: "LREAL", direction: "read",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
  };
}

/**
 * The tank loop with the given Smith seed as its program's own law. Source: smith_loop in
 * crates/trust-control/src/loop_definition/project_smith_tests.rs (lines 56-83), which takes complete_loop
 * (crates/trust-control/src/loop_definition/tests.rs lines 24-58), removes its law, sets design to project, and binds the
 * model's gain, time constant and delay, and the library block's operating input and output, as parameters. The helper binds
 * all five of those symbols, as the test helper does, whether or not the block is named.
 */
function smithTankLoop(seed: Record<string, unknown>): Record<string, unknown> {
  return {
    schemaVersion: 2,
    id: "loop:tank",
    name: "Tank level",
    revision: "sha256:loop-1",
    projectId: "project:sha256:abc",
    runtimeId: "runtime:local",
    controller: {
      instance: "Main.TankController",
      type: "Control.PID",
      revision: "sha256:controller-1",
      action: "direct",
      taskPeriodMs: 10,
      modeSignal: "%MX0.0",
      trackingSignal: "%ML24",
      design: "project",
      project: {
        family: "Smith",
        convention: "project-owned-native-controller",
        initialization: { kind: "declared-state-v1", states: [] },
        seed,
        axes: [],
        objective: { requirementId: "target-error", direction: "minimize" },
        maximumCandidates: 1,
        maximumCycles: 50000,
        wallBudgetMs: 60000,
      },
    },
    parameterBinding: {
      loop_id: "loop:tank",
      task_name: "MainTask",
      parameters: [
        {
          symbol: "Main.Parameters.Kp", iec_type: "LREAL", unit: "%/%",
          convention: "proportional gain", minimum: 0, maximum: 100,
        },
        {
          symbol: "Main.Parameters.Ki", iec_type: "LREAL", unit: "%/(%*s)",
          convention: "integral gain", minimum: 0, maximum: 100,
        },
        {
          symbol: "Main.Parameters.Kd", iec_type: "LREAL", unit: "%*s/%",
          convention: "derivative gain", minimum: 0, maximum: 100,
        },
        {
          symbol: "Main.Parameters.ModelGain", iec_type: "LREAL", unit: "degC/%",
          convention: "Smith predictor model gain", minimum: -1000000, maximum: 1000000,
        },
        {
          symbol: "Main.Parameters.ModelTau", iec_type: "LREAL", unit: "s",
          convention: "Smith predictor model time constant", minimum: 0.001, maximum: 100000,
        },
        {
          symbol: "Main.Parameters.ModelDelay", iec_type: "LREAL", unit: "s",
          convention: "Smith predictor model delay", minimum: 0, maximum: 10000,
        },
        {
          symbol: "Main.Parameters.OperatingInput", iec_type: "LREAL", unit: "%",
          convention: "Smith predictor operating input", minimum: 0, maximum: 100,
        },
        {
          symbol: "Main.Parameters.OperatingOutput", iec_type: "LREAL", unit: "degC",
          convention: "Smith predictor operating output", minimum: 0, maximum: 80,
        },
      ],
      mode: { symbol: "Main.Auto", manual_value: false },
    },
    signals: tankSignals(),
    limits: { outputMinimum: 0, outputMaximum: 100, rateUpPerS: 20, rateDownPerS: 25 },
    observedSampleTiming: { nominalPeriodMs: 10, maximumJitterMs: 1 },
    plantId: "tank",
    operatingPoint: "Normal production",
  };
}

/**
 * The Smith predictor without the library block (C9): a smith-pid seed on the PI gains and the model's gain, time constant
 * and delay. Source: smith_seed in crates/trust-control/src/loop_definition/project_smith_tests.rs (lines 35-54, without
 * the library keys), used by the test the_seed_without_the_library_block_is_served_with_the_model_values_alone (lines 113-132).
 * The decoder is crates/trust-control/src/loop_definition/project/smith_seed.rs (lines 13-65).
 */
export function smithLoop(): Record<string, unknown> {
  return smithTankLoop({
    kind: "smith-pid",
    family: "PI",
    proportionalSymbol: "Main.Parameters.Kp",
    integralSymbol: "Main.Parameters.Ki",
    derivativeSymbol: "Main.Parameters.Kd",
    gainSymbol: "Main.Parameters.ModelGain",
    timeConstantSymbol: "Main.Parameters.ModelTau",
    delaySymbol: "Main.Parameters.ModelDelay",
  });
}

/**
 * The Smith predictor with the library block Control.SmithPredictorFOPDT (C9): the four optional symbols are all named.
 * Source: smith_seed(true) in crates/trust-control/src/loop_definition/project_smith_tests.rs (lines 35-54), used by the test
 * a_complete_smith_seed_is_served_and_offers_only_the_pi_family (lines 94-111). The decoder is
 * crates/trust-control/src/loop_definition/project/smith_seed.rs (lines 13-65).
 */
export function smithLibraryLoop(): Record<string, unknown> {
  return smithTankLoop({
    kind: "smith-pid",
    family: "PI",
    proportionalSymbol: "Main.Parameters.Kp",
    integralSymbol: "Main.Parameters.Ki",
    derivativeSymbol: "Main.Parameters.Kd",
    gainSymbol: "Main.Parameters.ModelGain",
    timeConstantSymbol: "Main.Parameters.ModelTau",
    delaySymbol: "Main.Parameters.ModelDelay",
    predictorOutputSymbol: "Main.Predictor.Out",
    maximumCorrectionSymbol: "Main.Parameters.MaximumCorrection",
    operatingInputSymbol: "Main.Parameters.OperatingInput",
    operatingOutputSymbol: "Main.Parameters.OperatingOutput",
  });
}

// ---------------------------------------------------------------------------
// Gain schedule (C10): the Low point of the two-point tank schedule.
// ---------------------------------------------------------------------------

/** The Low point's seven retained states on channel 0, for the level in metres and the valve in percent. */
function gainScheduleStates(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Loop.StateValue.Setpoint", iec_type: "LREAL", unit: "m", minimum: 0, maximum: 2,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.SetpointRate", iec_type: "LREAL", unit: "m/s", minimum: -2000, maximum: 2000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.ProcessValue", iec_type: "LREAL", unit: "m", minimum: 0, maximum: 2,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.ProcessValueRate", iec_type: "LREAL", unit: "m/s", minimum: -2000, maximum: 2000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.FeedForward", iec_type: "LREAL", unit: "%", minimum: -100, maximum: 100,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.NominalOutput", iec_type: "LREAL", unit: "%", minimum: -1000, maximum: 1100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Loop.StateValue.SaturatedOutput", iec_type: "LREAL", unit: "%", minimum: 0, maximum: 100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
  ];
}

/**
 * The Low point's program block: family gain-scheduled, the PI seed on the Low gains, the schedule with both points, and the
 * startup flag as an application state. Source: project_block in crates/trust-control/src/setup/schedule_loop.rs (lines
 * 396-429), with the table of schedule_loop_tests.rs (lines 86-103) and the block read by gain_schedule_tests.rs (lines 13-26).
 */
function gainScheduleProject(): Record<string, unknown> {
  return {
    family: "gain-scheduled",
    convention: "project-owned-native-controller",
    initialization: {
      kind: "declared-state-v1",
      states: gainScheduleStates(),
      applicationStates: [
        {
          symbol: "P1.Started", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1,
          initial: 1, postScan: "unchanged",
        },
      ],
    },
    seed: { kind: "parallel-pid", family: "PI", proportionalSymbol: "P1.KpLow", integralSymbol: "P1.KiLow" },
    axes: [],
    objective: { requirementId: "target-error", direction: "minimize" },
    maximumCandidates: 1,
    maximumCycles: 50000,
    wallBudgetMs: 60000,
    schedule: {
      parameterRecord: "P1.Parameters",
      scheduling: { symbol: "P1.LevelM", unit: "m", iecType: "LREAL" },
      point: "Low",
      points: [
        {
          name: "Low", loopId: "loop:tank-level-low", value: 0.09, tolerance: 0.01,
          proportionalSymbol: "P1.KpLow", integralSymbol: "P1.KiLow",
        },
        {
          name: "High", loopId: "loop:tank-level-high", value: 0.49, tolerance: 0.01,
          proportionalSymbol: "P1.KpHigh", integralSymbol: "P1.KiHigh",
        },
      ],
    },
  };
}

/**
 * The Low point's loop of the tank's gain schedule (C10). Source: the point loop of schedule_loop.rs (point_loop, lines
 * 283-347; point_controller, lines 349-391; signals, lines 431-447; gain parameters, lines 449-492), on the tank catalog and draft
 * of crates/trust-control/src/setup/schedule_loop_tests.rs (tank_json lines 45-70, draft lines 105-135). The per-point assertions
 * at lines 155-211 check the seed (lines 176-184) and the two parameters with their units (lines 198-207). The schedule block
 * and the seed are those of crates/trust-control/src/loop_definition/project/gain_schedule_tests.rs (tank_block, lines 13-26).
 */
export function gainScheduleLoop(): Record<string, unknown> {
  return {
    schemaVersion: 2,
    id: "loop:tank-level-low",
    name: "Tank level · Low",
    revision: "sha256:tank-level-low",
    projectId: "project:setup",
    runtimeId: "runtime-1",
    controller: {
      instance: "P1.Loop",
      type: "Control.PID",
      revision: "sha256:unverified",
      action: "direct",
      taskPeriodMs: 100,
      design: "project",
      modeSignal: "P1.Auto",
      modeWhenTrue: "Auto",
      manualOutputSignal: "P1.ManualPct",
      manualOutputAddress: "%ML8",
      manualOutputType: "LREAL",
      project: gainScheduleProject(),
    },
    parameterBinding: {
      loop_id: "loop:tank-level-low",
      task_name: "MainTask",
      parameters: [
        {
          symbol: "P1.KpLow", iec_type: "LREAL", unit: "%/m",
          convention: "Scheduled proportional gain at Low", minimum: -1000000, maximum: 1000000,
        },
        {
          symbol: "P1.KiLow", iec_type: "LREAL", unit: "%/(m*s)",
          convention: "Scheduled integral gain at Low", minimum: -1000000, maximum: 1000000,
        },
      ],
      mode: { symbol: "P1.Auto", manual_value: false },
    },
    signals: {
      setpoint: {
        symbol: "P1.SetpointM", address: "%ML0", iecType: "LREAL", direction: "read",
        unit: "m", range: { minimum: 0, maximum: 2 },
      },
      processVariable: {
        symbol: "P1.LevelM", address: "%IL0", iecType: "LREAL", direction: "read",
        unit: "m", range: { minimum: 0, maximum: 2 },
      },
      manipulatedVariable: {
        symbol: "P1.ValvePct", address: "%QL0", iecType: "LREAL", direction: "write",
        unit: "%", range: { minimum: 0, maximum: 100 },
      },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    topology: "closed-loop",
  };
}

// ---------------------------------------------------------------------------
// Switching controller (C3): the cold room.
// ---------------------------------------------------------------------------

/**
 * The cold room's switching controller (C3): P1.CompressorPct is switched between 0 and 100 percent, with the equipment's
 * minimum ON and OFF times of 20 s and 30 s. Source: cold_room in crates/trust-control/src/study/tune/design/on_off_tests.rs
 * (lines 16-60) with actuator(Some((20.0, 30.0))) (lines 62-70) and the convention half-band, which the setup draft of
 * crates/trust-control/src/loop_definition/on_off_tests.rs (lines 13-42) also uses. The test decodes this loop. The block
 * fields and the states are those of on_off_tests.rs and of crates/trust-control/src/setup/on_off_loop.rs (controller_json,
 * lines 161-210). The parameter binding is the one cold_room writes; its conventions and bounds match on_off_loop.rs
 * (parameter_binding_json and setting, lines 257-288). thresholdEquality and noiseBand are not written: the defaults hold.
 */
export function onOffLoop(): Record<string, unknown> {
  return {
    schemaVersion: 2,
    id: "loop:cold-room",
    name: "Cold room",
    revision: "sha256:cold-1",
    projectId: "project:setup",
    runtimeId: "runtime-1",
    controller: {
      instance: "P1",
      type: "Main",
      revision: "sha256:controller-1",
      action: "reverse",
      taskPeriodMs: 100,
      design: "project",
      modeSignal: "P1.AutoMode",
      modeWhenTrue: "Auto",
      manualOutputSignal: "P1.ManualPct",
      manualOutputType: "LREAL",
      onOff: {
        hysteresisSymbol: "P1.HysteresisC",
        minimumOnTimeSymbol: "P1.MinimumOnS",
        minimumOffTimeSymbol: "P1.MinimumOffS",
        bandConvention: "half-band",
        lowOutput: 0,
        highOutput: 100,
        initialization: {
          kind: "declared-state-v1",
          states: [
            {
              symbol: "P1.CoolingOn", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1,
              initial: { source: "model-input-is", index: 0, value: 100 }, postScan: "bounded",
            },
            {
              symbol: "P1.DwellS", iec_type: "LREAL", unit: "s", minimum: 0, maximum: 1000000,
              initial: 0, postScan: "bounded",
            },
          ],
        },
      },
    },
    parameterBinding: {
      loop_id: "loop:cold-room",
      task_name: "MainTask",
      parameters: [
        {
          symbol: "P1.HysteresisC", iec_type: "LREAL", unit: "degC",
          convention: "ON/OFF hysteresis", minimum: 0, maximum: 30,
        },
        {
          symbol: "P1.MinimumOnS", iec_type: "LREAL", unit: "s",
          convention: "ON/OFF minimum ON time", minimum: 0, maximum: 3600,
        },
        {
          symbol: "P1.MinimumOffS", iec_type: "LREAL", unit: "s",
          convention: "ON/OFF minimum OFF time", minimum: 0, maximum: 3600,
        },
      ],
      mode: { symbol: "P1.AutoMode", manual_value: false },
    },
    signals: {
      setpoint: {
        symbol: "P1.SetpointC", iecType: "LREAL", direction: "read",
        unit: "degC", range: { minimum: -10, maximum: 20 },
      },
      processVariable: {
        symbol: "P1.RoomC", address: "%IL0", iecType: "LREAL", direction: "read",
        unit: "degC", range: { minimum: -10, maximum: 20 },
      },
      manipulatedVariable: {
        symbol: "P1.CompressorPct", address: "%QL0", iecType: "LREAL", direction: "write",
        unit: "%", range: { minimum: 0, maximum: 100 },
      },
    },
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    channels: {
      inputs: [
        {
          modelChannelId: "P1.CompressorPct",
          signalId: "manipulatedVariable",
          actuator: { kind: "on-off", minimum: 0, maximum: 100, minimumOnSeconds: 20, minimumOffSeconds: 30 },
        },
      ],
      outputs: [{ modelChannelId: "P1.RoomC", signalId: "processVariable", setpointSignalId: "setpoint" }],
    },
  };
}

// ---------------------------------------------------------------------------
// PID group (C7): two zones on the decentralized seed.
// ---------------------------------------------------------------------------

/** Zone 1's seven retained states on channel 0 (pid_states for the 20 to 60 degC measurement and the heater output). */
function groupZone1States(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Zone1.StateValue.Setpoint", iec_type: "LREAL", unit: "degC", minimum: 20, maximum: 60,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.SetpointRate", iec_type: "LREAL", unit: "degC/s", minimum: -40000, maximum: 40000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.ProcessValue", iec_type: "LREAL", unit: "degC", minimum: 20, maximum: 60,
      initial: { source: "model-output", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.ProcessValueRate", iec_type: "LREAL", unit: "degC/s", minimum: -40000, maximum: 40000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.FeedForward", iec_type: "LREAL", unit: "%", minimum: -100, maximum: 100,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.NominalOutput", iec_type: "LREAL", unit: "%", minimum: -1000, maximum: 1100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone1.StateValue.SaturatedOutput", iec_type: "LREAL", unit: "%", minimum: 0, maximum: 100,
      initial: { source: "model-input", index: 0 }, postScan: "bounded",
    },
  ];
}

/** Zone 2's seven retained states on channel 1 (the same bounds as zone 1, on the second input and output). */
function groupZone2States(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Zone2.StateValue.Setpoint", iec_type: "LREAL", unit: "degC", minimum: 20, maximum: 60,
      initial: { source: "model-output", index: 1 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.SetpointRate", iec_type: "LREAL", unit: "degC/s", minimum: -40000, maximum: 40000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.ProcessValue", iec_type: "LREAL", unit: "degC", minimum: 20, maximum: 60,
      initial: { source: "model-output", index: 1 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.ProcessValueRate", iec_type: "LREAL", unit: "degC/s", minimum: -40000, maximum: 40000,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.FeedForward", iec_type: "LREAL", unit: "%", minimum: -100, maximum: 100,
      initial: 0, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.NominalOutput", iec_type: "LREAL", unit: "%", minimum: -1000, maximum: 1100,
      initial: { source: "model-input", index: 1 }, postScan: "bounded",
    },
    {
      symbol: "P1.Zone2.StateValue.SaturatedOutput", iec_type: "LREAL", unit: "%", minimum: 0, maximum: 100,
      initial: { source: "model-input", index: 1 }, postScan: "bounded",
    },
  ];
}

/**
 * The group's program block: family paired-PI, the decentralized seed, the members' states with the startup flag, and the
 * objective of the first member's measurement. Source: group_project in crates/trust-control/src/setup/group_loop.rs
 * (lines 514-552).
 */
function groupProject(): Record<string, unknown> {
  return {
    family: "paired-PI",
    convention: "project-owned-native-controller",
    initialization: {
      kind: "declared-state-v1",
      states: [
        ...groupZone1States(),
        ...groupZone2States(),
        {
          symbol: "P1.Initialized", iec_type: "BOOL", unit: "1", minimum: 0, maximum: 1,
          initial: 1, postScan: "unchanged",
        },
      ],
    },
    seed: { kind: "decentralized-pid", family: "PI" },
    axes: [],
    objective: { requirementId: "P1.Zone1C:target-error", direction: "minimize" },
    maximumCandidates: 1,
    maximumCycles: 50000,
    wallBudgetMs: 60000,
  };
}

/**
 * The group's two members, each with its channels, the shared mode, its gains and its retained states (the first member also
 * owns the startup flag). Source: group_block in crates/trust-control/src/setup/group_loop.rs (lines 554-590); the member shape
 * is that of crates/trust-control/src/loop_definition/pid_group_tests.rs (member, lines 27-43).
 */
function groupMembers(): Record<string, unknown>[] {
  return [
    {
      instance: "P1.Zone1",
      type: "Control.PID",
      inputChannelId: "P1.Heater1Pct",
      outputChannelId: "P1.Zone1C",
      modeSymbol: "P1.ZonesAuto",
      parameterSymbols: [
        "P1.Zone1Parameters.ProportionalGain",
        "P1.Zone1Parameters.IntegralGain",
        "P1.Zone1Parameters.DerivativeGain",
      ],
      applicationStateSymbols: [
        "P1.Zone1.StateValue.Setpoint",
        "P1.Zone1.StateValue.SetpointRate",
        "P1.Zone1.StateValue.ProcessValue",
        "P1.Zone1.StateValue.ProcessValueRate",
        "P1.Zone1.StateValue.FeedForward",
        "P1.Zone1.StateValue.NominalOutput",
        "P1.Zone1.StateValue.SaturatedOutput",
        "P1.Initialized",
      ],
    },
    {
      instance: "P1.Zone2",
      type: "Control.PID",
      inputChannelId: "P1.Heater2Pct",
      outputChannelId: "P1.Zone2C",
      modeSymbol: "P1.ZonesAuto",
      parameterSymbols: [
        "P1.Zone2Parameters.ProportionalGain",
        "P1.Zone2Parameters.IntegralGain",
        "P1.Zone2Parameters.DerivativeGain",
      ],
      applicationStateSymbols: [
        "P1.Zone2.StateValue.Setpoint",
        "P1.Zone2.StateValue.SetpointRate",
        "P1.Zone2.StateValue.ProcessValue",
        "P1.Zone2.StateValue.ProcessValueRate",
        "P1.Zone2.StateValue.FeedForward",
        "P1.Zone2.StateValue.NominalOutput",
        "P1.Zone2.StateValue.SaturatedOutput",
      ],
    },
  ];
}

/** The six gains of the two members' records, zone 1 first (groupParameters in group_loop.rs; the decoupler entries are not used). */
function groupParameters(): Record<string, unknown>[] {
  return [
    {
      symbol: "P1.Zone1Parameters.ProportionalGain", iec_type: "LREAL", unit: "%/degC",
      convention: "Control.PID ProportionalGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.Zone1Parameters.IntegralGain", iec_type: "LREAL", unit: "%/(degC*s)",
      convention: "Control.PID IntegralGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.Zone1Parameters.DerivativeGain", iec_type: "LREAL", unit: "%*s/degC",
      convention: "Control.PID DerivativeGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.Zone2Parameters.ProportionalGain", iec_type: "LREAL", unit: "%/degC",
      convention: "Control.PID ProportionalGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.Zone2Parameters.IntegralGain", iec_type: "LREAL", unit: "%/(degC*s)",
      convention: "Control.PID IntegralGain", minimum: -1000000, maximum: 1000000,
    },
    {
      symbol: "P1.Zone2Parameters.DerivativeGain", iec_type: "LREAL", unit: "%*s/degC",
      convention: "Control.PID DerivativeGain", minimum: -1000000, maximum: 1000000,
    },
  ];
}

/** The first member's signals as the loop's primary ones, and the second member's three on its own channel (group_signals, member_signals). */
function groupSignals(): Record<string, unknown> {
  return {
    setpoint: {
      symbol: "P1.Zone1SetpointC", address: "%ML0", iecType: "LREAL", direction: "read",
      unit: "degC", range: { minimum: 20, maximum: 60 },
    },
    processVariable: {
      symbol: "P1.Zone1C", address: "%IL0", iecType: "LREAL", direction: "read",
      unit: "degC", range: { minimum: 20, maximum: 60 },
    },
    manipulatedVariable: {
      symbol: "P1.Heater1Pct", address: "%QL0", iecType: "LREAL", direction: "write",
      unit: "%", range: { minimum: 0, maximum: 100 },
    },
    additional: [
      {
        id: "mv-2", channel: "member-2", role: "MV",
        symbol: "P1.Heater2Pct", address: "%QL8", iecType: "LREAL", direction: "write",
        unit: "%", range: { minimum: 0, maximum: 100 },
      },
      {
        id: "pv-2", channel: "member-2", role: "PV",
        symbol: "P1.Zone2C", address: "%IL8", iecType: "LREAL", direction: "read",
        unit: "degC", range: { minimum: 20, maximum: 60 },
      },
      {
        id: "sp-2", channel: "member-2", role: "SP",
        symbol: "P1.Zone2SetpointC", address: "%ML8", iecType: "LREAL", direction: "read",
        unit: "degC", range: { minimum: 20, maximum: 60 },
      },
    ],
  };
}

/** Two inputs, one per heater, and two outputs, one per measurement, each with its setpoint (group_channels in group_loop.rs). */
function groupChannels(): Record<string, unknown> {
  return {
    inputs: [
      {
        modelChannelId: "P1.Heater1Pct",
        signalId: "manipulatedVariable",
        actuator: { kind: "continuous", minimum: 0, maximum: 100 },
        experimentStep: 10,
      },
      {
        modelChannelId: "P1.Heater2Pct",
        signalId: "mv-2",
        manualOutputSignal: "P1.Zone2ManualPct",
        manualOutputType: "LREAL",
        actuator: { kind: "continuous", minimum: 0, maximum: 100 },
        experimentStep: 10,
      },
    ],
    outputs: [
      { modelChannelId: "P1.Zone1C", signalId: "processVariable", setpointSignalId: "setpoint" },
      { modelChannelId: "P1.Zone2C", signalId: "pv-2", setpointSignalId: "sp-2" },
    ],
  };
}

/**
 * A two-zone PID group (C7) with the decentralized seed. Source: the group of draft() in
 * crates/trust-control/src/setup/group_loop_tests.rs (lines 139-171, with the zones of lines 69-82 and the catalog of
 * lines 106-132), which the test the_two_member_group_is_written_read_back_and_accepted decodes (lines 213-227). The loop is
 * written by group_loop_definition in crates/trust-control/src/setup/group_loop.rs (lines 135-199). The decoupled variant of
 * the generator test (decoupler(), lines 173-179) is not included. The group declaration is decoded by
 * crates/trust-control/src/loop_definition/pid_group.rs (lines 30-133).
 */
export function pidGroupLoop(): Record<string, unknown> {
  return {
    schemaVersion: 2,
    id: "loop:zone-temperatures",
    name: "Zone temperatures",
    revision: "sha256:zone-temperatures",
    projectId: "project:setup",
    runtimeId: "runtime-1",
    controller: {
      instance: "P1.Zone1",
      type: "Control.PID",
      revision: "sha256:unverified",
      action: "direct",
      taskPeriodMs: 100,
      design: "project",
      modeSignal: "P1.ZonesAuto",
      modeWhenTrue: "Auto",
      manualOutputSignal: "P1.Zone1ManualPct",
      manualOutputAddress: "%ML16",
      manualOutputType: "LREAL",
      project: groupProject(),
      group: { members: groupMembers() },
    },
    parameterBinding: {
      loop_id: "loop:zone-temperatures",
      task_name: "MainTask",
      parameters: groupParameters(),
      mode: { symbol: "P1.ZonesAuto", manual_value: false },
    },
    signals: groupSignals(),
    limits: { outputMinimum: 0, outputMaximum: 100 },
    observedSampleTiming: { nominalPeriodMs: 100 },
    topology: "closed-loop",
    experiment: { execution: "control-studio", kind: "mv_step", amplitude: 10, durationSeconds: 40, seed: 1 },
    channels: groupChannels(),
  };
}
