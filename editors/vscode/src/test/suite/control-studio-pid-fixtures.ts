import * as fs from "fs";
import * as path from "path";

export function pidLoop(): any {
  const root = path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json");
  const loop = JSON.parse(fs.readFileSync(root, "utf8"));
  loop.controller = { ...loop.controller, type: "Control.PID", design: "pid", law: {
    kind: "pid", parameterRecordSymbol: "Main.Parameters", parameterRevisionSymbol: "Main.ParameterRevision",
    proportionalGainSymbol: "Main.Kp", integralGainSymbol: "Main.Ki", derivativeGainSymbol: "Main.Kd",
  } };
  loop.parameterBinding = { ...loop.parameterBinding, parameters: [
    { symbol: "Main.Kp", iec_type: "LREAL", unit: "V/m", convention: "proportional gain", minimum: 0, maximum: 100 },
    { symbol: "Main.Ki", iec_type: "LREAL", unit: "V/(m*s)", convention: "integral gain", minimum: 0, maximum: 100 },
    { symbol: "Main.Kd", iec_type: "LREAL", unit: "V*s/m", convention: "derivative gain", minimum: 0, maximum: 100 },
  ] };
  return loop;
}

export function proportionalLoop(): any {
  const root = path.resolve(__dirname, "../../../src/test/fixtures/control-studio-engineer/control-loop.json");
  return JSON.parse(fs.readFileSync(root, "utf8"));
}

export function channel(id: string, unit: string) { return { id, unit, operatingRange: { minimum: -20, maximum: 20 } }; }

export function pidInitialization() {
  return { kind: "parallel-pid-v1", integralState: { symbol: "Main.Integral", iecType: "LREAL", unit: "V", minimum: -20, maximum: 20 },
    derivativeState: { symbol: "Main.PreviousError", iecType: "LREAL", unit: "m", minimum: -20, maximum: 20 },
    proportionalSetpointWeight: 1, derivativeSetpointWeight: 1, maximumSnapshotAgeMs: 100 };
}
