import { controllerAdapter } from "./controllerAdapters";
import { declaredControllerChannels } from "./declaredControllerChannels";
import type { DeclaredStateTracking } from "./controllerInitialization";
import type { ControlLoopDefinition } from "./loopDefinition";
import { RuntimeTransferClient, type TransferOwner, type TransferStatus } from "./runtimeTransfer";
import type { RuntimeRequest } from "./verificationClient";
import { noiseAwareTransferTolerance, type TransferBinding } from "./proportionalTransfer";
import { decodePidLaw } from "./controllerLaw";

export interface IdentificationModeCheckpoint {
  loopRevision: string;
  original: TransferStatus;
  transfer?: { binding: TransferBinding; owner: TransferOwner };
}
export interface IdentificationModePersistence {
  read(): IdentificationModeCheckpoint | undefined;
  write(value: IdentificationModeCheckpoint | undefined): Promise<void>;
}

/** Existing controller states track Manual in ST; only guarded mode/command transactions grant authority. */
export class ConnectedIdentificationMode {
  /** Half the range of the measurement at the identification's steady baseline, once observed. */
  processNoise: number | undefined;
  private readonly binding: TransferBinding;
  private checkpoint: IdentificationModeCheckpoint | undefined;
  constructor(private readonly loop: ControlLoopDefinition, private readonly request: RuntimeRequest,
    private readonly persistence: IdentificationModePersistence) {
    const binding = controllerAdapter(loop).transferBinding();
    this.binding = { ...binding, loop_id: `${binding.loop_id}:identification-mode` };
    this.checkpoint = persistence.read();
  }
  async enter(manual: boolean): Promise<void> {
    if (this.checkpoint) await this.restore();
    const status = await this.status(this.binding);
    if (status.values[this.binding.mode.symbol] === (manual ? this.binding.mode.manual_value : !this.binding.mode.manual_value)) return;
    this.checkpoint = { loopRevision: this.loop.revision, original: status };
    await this.persistence.write(this.checkpoint);
    await this.transfer(this.binding, this.stateValues(this.binding, status), manual, this.tracking(status), status);
  }
  async restore(): Promise<void> {
    const checkpoint = this.checkpoint;
    if (!checkpoint) return;
    if (checkpoint.loopRevision !== this.loop.revision) throw new Error("Restore the original loop bindings before recovering identification mode.");
    if (checkpoint.transfer) {
      const client = new RuntimeTransferClient(checkpoint.transfer.binding, checkpoint.original.program_revision, this.request);
      client.restoreOwner(checkpoint.transfer.owner);
      await client.cancel(); // Reconcile this ID; never repeat its Apply.
      checkpoint.transfer = undefined; await this.persistence.write(checkpoint);
    }
    let current = await this.status(this.binding);
    if (current.program_revision !== checkpoint.original.program_revision) throw new Error("The program changed; the saved identification mode cannot be restored to this program.");
    if (current.values[this.binding.mode.symbol] !== this.binding.mode.manual_value) {
      await this.transfer(this.binding, this.stateValues(this.binding, current), true, this.tracking(current));
    }
    // In Manual the ST holds its own evolving PID states. Restore commands without replacing that state history.
    const channels = declaredControllerChannels(this.loop);
    const toAuto = checkpoint.original.values[this.binding.mode.symbol] !== this.binding.mode.manual_value;
    // Returning to Auto, a manual output the code sets from the output only holds the output of the moment the checkpoint was
    // taken: Auto overwrites it, and writing it back would move the output (§17.1). The references return.
    const commands = channels.outputs.filter((_, index) => !(toAuto && index === 0 && this.loop.controller.manualOutputFollowsOutput))
      .map(channel => channel.command);
    const controls = [...channels.references.map(channel => channel.reference), ...commands];
    const commandBinding = { ...this.binding, loop_id: `${this.binding.loop_id}:commands`, states: controls,
      guards: this.binding.guards.filter(member => this.loop.parameterBinding?.parameters.some(parameter => parameter.symbol === member.symbol)) };
    if (controls.length) {
      await this.transfer(commandBinding, Object.fromEntries(controls.map(member => [member.symbol, Number(checkpoint.original.values[member.symbol])])), true);
    }
    if (toAuto) {
      current = await this.status(this.binding);
      await this.transfer(this.binding, this.stateValues(this.binding, current), false, this.tracking(current), current);
    }
    this.checkpoint = undefined; await this.persistence.write(undefined);
  }
  private async status(binding: TransferBinding): Promise<TransferStatus> {
    const raw = await this.request("control.transfer.status", { binding }) as TransferStatus;
    return new RuntimeTransferClient(binding, raw.program_revision, this.request).status();
  }
  private stateValues(binding: TransferBinding, status: TransferStatus): Record<string, number> {
    return Object.fromEntries(binding.states.map(member => [member.symbol, Number(status.values[member.symbol])]));
  }
  /** An Auto entry's first output moves by the PID's own response of one cycle (§17.1): its integral step on the error it enters
   * with, which identification has moved the measurement away from the setpoint to make, and its proportional and derivative
   * response to the measurement noise the identification saw. */
  private autoEntryResponse(observed: TransferStatus): number {
    const pid = decodePidLaw(this.loop.controller.law, this.loop.parameterBinding);
    if (!pid) return 0;
    const value = (symbol: string) => Number(observed.values[symbol]);
    const cycleSeconds = this.loop.controller.taskPeriodMs / 1000;
    const reference = declaredControllerChannels(this.loop).references[0].reference.symbol;
    const error = Math.abs(value(reference) - value(this.loop.signals.processVariable.symbol));
    const response = noiseAwareTransferTolerance(this.loop, undefined, value(pid.proportionalGainSymbol), this.processNoise,
      { integralGain: value(pid.integralGainSymbol), derivativeGain: value(pid.derivativeGainSymbol), cycleSeconds })
      + Math.abs(value(pid.integralGainSymbol)) * cycleSeconds * error;
    return Number.isFinite(response) ? response : 0;
  }
  /** Code that copies the output into the manual output changes it in every Auto scan, before a transfer to Manual and after
   * one to Auto, so the manual output is a bounded state either way (§17.1). */
  private tracking(status: TransferStatus): DeclaredStateTracking {
    const channels = declaredControllerChannels(this.loop);
    const controls = new Set([...channels.references.map(channel => channel.reference.symbol), ...channels.outputs.map(channel => channel.command.symbol)]);
    const dynamic = this.binding.states.filter(member => !controls.has(member.symbol));
    return { kind: "declared-state-v1", reference_symbol: channels.references[0].reference.symbol,
      manual_command_symbol: channels.outputs[0].command.symbol, process_variable: this.loop.signals.processVariable.symbol,
      align_output: false, initialization_values: Object.fromEntries(dynamic.map(member => [member.symbol, Number(status.values[member.symbol])])),
      bounded_state_symbols: [...dynamic.map(member => member.symbol),
        ...(this.loop.controller.manualOutputFollowsOutput ? [channels.outputs[0].command.symbol] : [])],
      additional_measurements: channels.measurements.map(member => member.symbol),
      additional_references: channels.references.slice(1).map(channel => ({ reference_symbol: channel.reference.symbol, process_variable: channel.process.symbol })),
      additional_commands: channels.outputs.slice(1).map(channel => ({ manual_command_symbol: channel.command.symbol, output_symbol: channel.output.symbol })) };
  }
  private async transfer(binding: TransferBinding, values: Record<string, number>, manual: boolean, tracking?: DeclaredStateTracking,
    observed?: TransferStatus): Promise<void> {
    const checkpoint = this.checkpoint!;
    const client = new RuntimeTransferClient(binding, checkpoint.original.program_revision, this.request);
    client.setOwnerPersistence(async owner => { checkpoint.transfer = { binding, owner }; await this.persistence.write(checkpoint); });
    const rate = Math.max(this.loop.limits.rateUpPerS ?? 0, this.loop.limits.rateDownPerS ?? 0);
    const tolerance = Math.max(1e-6, rate * this.loop.controller.taskPeriodMs / 1000, !manual && observed ? this.autoEntryResponse(observed) : 0);
    await client.apply(values, manual, tolerance, tracking);
    checkpoint.transfer = undefined; await this.persistence.write(checkpoint);
  }
}
