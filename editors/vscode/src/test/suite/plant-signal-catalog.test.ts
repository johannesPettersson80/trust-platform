import * as assert from "assert";

import * as plantSignals from "../../networkCanvas/plant/signalCatalog";

suite("Plant signal catalog", () => {
  test("keeps compiled typed candidates offline and overlays matching live values", () => {
    const compiled = {
      ok: true,
      command: "trust-lsp.hmiBindings",
      processImage: {
        commands: [{ symbol: "P1.ManipulatedValue", address: "%QL0", type: "LREAL" }],
        feedbacks: [
          { symbol: "P1.ProcessValue", address: "%IL0", type: "LREAL" },
          { symbol: "P1.OptionalFeedback", address: "%IL8", type: "LREAL" },
        ],
      },
    };

    assert.deepStrictEqual(
      plantSignals.buildPlantSignalCatalog(compiled, {
        outputs: [
          {
            name: "P1.ManipulatedValue",
            address: "%QL0",
            valueType: "LREAL",
            value: "12.5",
          },
        ],
        inputs: [
          {
            name: "P1.ProcessValue",
            address: "%IL0",
            valueType: "LREAL",
            value: "8.25",
          },
        ],
        memory: [],
      }),
      {
        status: "ready",
        commands: [
          {
            symbol: "P1.ManipulatedValue",
            address: "%QL0",
            dataType: "LREAL",
            currentValue: "12.5",
          },
        ],
        feedbacks: [
          {
            symbol: "P1.ProcessValue",
            address: "%IL0",
            dataType: "LREAL",
            currentValue: "8.25",
          },
          {
            symbol: "P1.OptionalFeedback",
            address: "%IL8",
            dataType: "LREAL",
          },
        ],
      },
    );
  });

  test("a memory value is a command source too, as a disturbance the HMI sets (spec 33, §13.1)", () => {
    const compiled = {
      ok: true,
      processImage: {
        commands: [{ symbol: "Main.HeaterPower", address: "%QL8", type: "LREAL" }],
        feedbacks: [{ symbol: "Main.TankTemperature", address: "%IL8", type: "LREAL" }],
        memory: [{ symbol: "Simulation.WindowOpening", address: "%ML0", type: "LREAL" }],
      },
    };
    const catalog = plantSignals.buildPlantSignalCatalog(compiled, { inputs: [], outputs: [], memory: [] });
    assert.deepStrictEqual(
      catalog.commands.map((candidate) => candidate.address),
      ["%QL8", "%ML0"],
    );
    assert.deepStrictEqual(plantSignals.validateManualPlantSignalAddress(catalog, "command", "%ml0"), {
      ok: true,
      address: "%ML0",
    });
    assert.deepStrictEqual(plantSignals.validateManualPlantSignalAddress(catalog, "feedback", "%ML0"), {
      ok: false,
      code: "wrong_direction",
      message: "Use a compiled numeric %I input for feedback.",
    });
  });

  test("fails closed when the current project does not compile", () => {
    assert.deepStrictEqual(
      plantSignals.buildPlantSignalCatalog(
        { ok: false, error: "type mismatch at src/main.st:4" },
        {
          outputs: [{ address: "%QL0", value: "99", valueType: "LREAL" }],
          inputs: [],
          memory: [],
        },
      ),
      {
        status: "compile_error",
        commands: [],
        feedbacks: [],
        error: "type mismatch at src/main.st:4",
      },
    );
  });

  test("accepts only compiled picker candidates entered as manual addresses", () => {
    const validate = (
      plantSignals as unknown as {
        validateManualPlantSignalAddress?: (
          catalog: plantSignals.PlantSignalCatalog,
          direction: "command" | "feedback",
          address: string,
        ) => unknown;
      }
    ).validateManualPlantSignalAddress;
    assert.strictEqual(typeof validate, "function", "manual address validation must use the compiled picker catalog");
    if (!validate) return;

    const catalog: plantSignals.PlantSignalCatalog = {
      status: "ready",
      commands: [{ symbol: "P1.ManipulatedValue", address: "%QL0", dataType: "LREAL" }],
      feedbacks: [{ symbol: "P1.ProcessValue", address: "%IL0", dataType: "LREAL" }],
    };

    assert.deepStrictEqual(validate(catalog, "command", "  %ql0  "), {
      ok: true,
      address: "%QL0",
    });
    assert.deepStrictEqual(validate(catalog, "command", "%QL8"), {
      ok: false,
      code: "not_compiled",
      message: "Enter or choose a compiled numeric %Q output or %M memory value.",
    });
  });

  test("reports duplicate, wrong-direction, absent, and enabled physical I/O conflicts", () => {
    const api = plantSignals as unknown as {
      enabledPhysicalIoDrivers?: (schema: unknown) => string[];
      validateManualPlantSignalAddress: (
        catalog: plantSignals.PlantSignalCatalog,
        direction: "command" | "feedback",
        address: string,
        context?: { usedFeedbackTargets?: string[] },
      ) => unknown;
    };
    assert.strictEqual(
      typeof api.enabledPhysicalIoDrivers,
      "function",
      "physical ownership must come from the existing communication schema",
    );
    if (!api.enabledPhysicalIoDrivers) return;
    const physicalDrivers = api.enabledPhysicalIoDrivers({
      schema_version: 4,
      protocols: [
        {
          id: "gpio",
          title: "GPIO",
          category: "field_device",
          instances: [{ id: "gpio:0", enabled: true }],
        },
        {
          id: "modbus_tcp",
          title: "Modbus TCP",
          category: "field_device",
          instances: [{ id: "modbus_tcp:1", enabled: false }],
        },
        {
          id: "simulated",
          title: "Simulated I/O",
          category: "field_device",
          instances: [{ id: "simulated:2", enabled: true }],
        },
      ],
    });
    assert.deepStrictEqual(physicalDrivers, ["GPIO"]);

    const catalog: plantSignals.PlantSignalCatalog = {
      status: "ready",
      commands: [{ symbol: "P1.ManipulatedValue", address: "%QL0", dataType: "LREAL" }],
      feedbacks: [{ symbol: "P1.ProcessValue", address: "%IL0", dataType: "LREAL" }],
    };
    assert.deepStrictEqual(api.validateManualPlantSignalAddress(catalog, "command", "%IL0"), {
      ok: false,
      code: "wrong_direction",
      message: "Use a compiled numeric %Q output or %M memory value for a command.",
    });
    assert.deepStrictEqual(
      api.validateManualPlantSignalAddress(catalog, "feedback", "%IL0", {
        usedFeedbackTargets: ["%il0"],
      }),
      {
        ok: false,
        code: "duplicate_feedback",
        message: "Choose a different %I input; %IL0 is already used by another plant.",
      },
    );
    assert.deepStrictEqual(api.validateManualPlantSignalAddress(catalog, "feedback", "%IL8"), {
      ok: false,
      code: "not_compiled",
      message: "Enter or choose a compiled numeric %I input.",
    });
    assert.deepStrictEqual(
      api.validateManualPlantSignalAddress(
        { ...catalog, physicalDrivers } as unknown as plantSignals.PlantSignalCatalog,
        "feedback",
        "%IL0",
      ),
      {
        ok: false,
        code: "physical_io_enabled",
        message:
          "%IL0 is owned by enabled physical I/O (GPIO), so simulated feedback cannot write it. Disable GPIO in Devices & Connections or choose another compiled %I input.",
      },
    );
  });
});
