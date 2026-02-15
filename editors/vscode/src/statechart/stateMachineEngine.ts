/**
 * Simple in-memory state machine simulator
 * Supports both simulation (mock) and hardware (real I/O) execution modes
 */

import type { ExecutionState } from "./webview/types";
import type { RuntimeClient } from "./runtimeClient";

type ExecutionMode = "simulation" | "hardware";

interface ActionMapping {
  action: string;
  address?: string;
  variable?: string;
  value?: any;
  message?: string;
  targets?: Array<{ address: string; value: any }>;
}

interface StateMachineConfig {
  id: string;
  initial?: string;
  states: Record<string, StateConfig>;
  actionMappings?: Record<string, ActionMapping>;
}

interface StateConfig {
  entry?: string[];
  exit?: string[];
  on?: Record<string, TransitionConfig | string>;
  type?: "final" | "compound";
  after?: number; // Delay in milliseconds before auto-transition
}

interface TransitionConfig {
  target: string;
  guard?: string;
  actions?: string[];
  after?: number; // Delay in milliseconds before this transition
}

export class StateMachineEngine {
  private config: StateMachineConfig;
  private currentState: string;
  private previousState: string | undefined;
  private context: Record<string, any> = {};
  private mode: ExecutionMode;
  private runtimeClient?: RuntimeClient;
  private forcedAddresses: Set<string> = new Set();
  private activeTimers: Map<string, NodeJS.Timeout> = new Map();
  private initialized = false;

  constructor(
    configJson: string,
    mode: ExecutionMode = "simulation",
    runtimeClient?: RuntimeClient
  ) {
    this.config = JSON.parse(configJson);
    this.currentState = this.config.initial || Object.keys(this.config.states)[0];
    this.mode = mode;
    this.runtimeClient = runtimeClient;

    if (!this.currentState) {
      throw new Error("No initial state defined");
    }
  }

  /**
   * Initialize machine by running entry actions and scheduling initial timers.
   */
  async initialize(): Promise<void> {
    if (this.initialized) {
      return;
    }

    console.log(`🎯 StateMachine initialized in ${this.mode} mode`);

    // Execute entry actions of initial state
    await this.executeActions(this.config.states[this.currentState]?.entry);

    // Schedule timers for initial state
    this.scheduleStateTimers(this.currentState);
    this.initialized = true;
  }

  private async ensureInitialized(): Promise<void> {
    if (!this.initialized) {
      await this.initialize();
    }
  }

  /**
   * Schedule automatic timers for a state
   */
  private scheduleStateTimers(stateName: string): void {
    const state = this.config.states[stateName];
    if (!state || !state.on) {
      return;
    }

    // Look for transitions with 'after' field
    for (const [eventName, transition] of Object.entries(state.on)) {
      const afterDelay = typeof transition === "object" ? transition.after : undefined;
      
      if (afterDelay !== undefined && afterDelay > 0) {
        console.log(`⏰ Scheduling auto-transition: ${stateName} --[${eventName}]--> (after ${afterDelay}ms)`);
        const timerId = setTimeout(() => {
          console.log(`🔥 Timer fired: ${eventName} on ${stateName} (${afterDelay}ms elapsed)`);
          void this.sendEvent(eventName).catch((error) => {
            console.error(`Failed to handle auto-transition event ${eventName}:`, error);
          });
        }, afterDelay);
        
        this.activeTimers.set(`${stateName}:${eventName}`, timerId);
      }
    }
  }

  /**
   * Cancel all timers for a state
   */
  private cancelStateTimers(stateName: string): void {
    const timersToCancel: string[] = [];
    
    for (const [key, timer] of this.activeTimers) {
      if (key.startsWith(`${stateName}:`)) {
        clearTimeout(timer);
        timersToCancel.push(key);
      }
    }
    
    if (timersToCancel.length > 0) {
      console.log(`🛑 Cancelled ${timersToCancel.length} timer(s) for state: ${stateName}`);
    }
    
    for (const key of timersToCancel) {
      this.activeTimers.delete(key);
    }
  }

  /**
   * Cleanup: release all forced I/O addresses and cancel timers
   */
  async cleanup(): Promise<void> {
    // Cancel all active timers
    for (const [eventName, timer] of this.activeTimers) {
      clearTimeout(timer);
    }
    this.activeTimers.clear();

    // Release forced I/O
    if (this.mode === "hardware" && this.runtimeClient && this.forcedAddresses.size > 0) {
      console.log(`🧹 Releasing ${this.forcedAddresses.size} forced addresses...`);
      for (const address of this.forcedAddresses) {
        try {
          await this.runtimeClient.unforceIo(address);
        } catch (error) {
          console.error(`Failed to unforce ${address}:`, error);
        }
      }
      this.forcedAddresses.clear();
    }
  }

  /**
   * Get current execution state
   */
  getExecutionState(): ExecutionState {
    return {
      currentState: this.currentState,
      previousState: this.previousState,
      availableEvents: this.getAvailableEvents(),
      context: { ...this.context },
      timestamp: Date.now(),
      mode: this.mode,
    };
  }

  /**
   * Send an event to trigger a transition
   */
  async sendEvent(eventName: string): Promise<boolean> {
    await this.ensureInitialized();

    const state = this.config.states[this.currentState];
    if (!state || !state.on) {
      console.log(`No transitions defined for state: ${this.currentState}`);
      return false;
    }

    const transition = state.on[eventName];
    if (!transition) {
      console.log(`Event ${eventName} not available in state: ${this.currentState}`);
      return false;
    }

    // Parse transition
    const targetState = typeof transition === "string" ? transition : transition.target;
    const actions = typeof transition === "object" ? transition.actions : undefined;
    const guard = typeof transition === "object" ? transition.guard : undefined;
    const newState = this.config.states[targetState];

    if (!newState) {
      console.warn(`Transition target "${targetState}" does not exist`);
      return false;
    }

    // Check guard
    if (guard && !(await this.evaluateGuard(guard))) {
      console.log(`Guard ${guard} blocked transition`);
      return false;
    }

    // Cancel timers from current state
    this.cancelStateTimers(this.currentState);

    // Execute exit actions from current state
    await this.executeActions(state.exit);

    // Execute transition actions
    await this.executeActions(actions);

    // Transition to new state
    this.previousState = this.currentState;
    this.currentState = targetState;

    // Execute entry actions of new state
    await this.executeActions(newState.entry);

    console.log(`Transitioned from ${this.previousState} to ${this.currentState} via ${eventName}`);

    // Schedule timers for new state
    this.scheduleStateTimers(targetState);

    return true;
  }

  /**
   * Get available events from current state
   */
  private getAvailableEvents(): string[] {
    const state = this.config.states[this.currentState];
    if (!state || !state.on) {
      return [];
    }
    return Object.keys(state.on);
  }

  /**
   * Execute a list of actions
   */
  private async executeActions(actions?: string[]): Promise<void> {
    if (!actions || actions.length === 0) {
      return;
    }

    for (const action of actions) {
      if (this.mode === "simulation") {
        // Simulation mode: just log
        console.log(`🖥️  [SIM] Executing action: ${action}`);
      } else {
        // Hardware mode: execute real actions
        await this.executeHardwareAction(action);
      }
    }
  }

  /**
   * Execute a hardware action using action mappings
   */
  private async executeHardwareAction(actionName: string): Promise<void> {
    if (!this.config.actionMappings) {
      console.warn(`⚠️  No actionMappings defined, cannot execute: ${actionName}`);
      return;
    }

    const mapping = this.config.actionMappings[actionName];
    if (!mapping) {
      console.warn(`⚠️  No mapping found for action: ${actionName}`);
      return;
    }

    if (!this.runtimeClient || !this.runtimeClient.isConnected()) {
      console.error(`❌ Runtime client not connected, cannot execute: ${actionName}`);
      return;
    }

    try {
      switch (mapping.action) {
        case "WRITE_OUTPUT":
          if (mapping.address !== undefined && mapping.value !== undefined) {
            console.log(`🔌 [HW] ${actionName} → FORCE ${mapping.value} on ${mapping.address}`);
            // Use io.force to override program outputs
            await this.runtimeClient.forceIo(mapping.address, mapping.value);
            this.forcedAddresses.add(mapping.address);
          }
          break;

        case "WRITE_VARIABLE":
          if (mapping.variable !== undefined && mapping.value !== undefined) {
            console.log(`🔌 [HW] ${actionName} → SET ${mapping.variable} = ${mapping.value}`);
            // Variables would be handled differently - for now just log
          }
          break;

        case "SET_MULTIPLE":
          if (mapping.targets) {
            console.log(`🔌 [HW] ${actionName} → FORCE MULTIPLE (${mapping.targets.length} outputs)`);
            for (const target of mapping.targets) {
              await this.runtimeClient.forceIo(target.address, target.value);
              this.forcedAddresses.add(target.address);
            }
          }
          break;

        case "LOG":
          console.log(`📝 [HW] ${actionName} → ${mapping.message || ""}`);
          break;

        default:
          console.warn(`⚠️  Unknown action type: ${mapping.action}`);
      }
    } catch (error) {
      console.error(`❌ Failed to execute hardware action ${actionName}:`, error);
    }
  }

  /**
   * Evaluate a guard condition
   * Supports:
   * - Simple I/O reads: %IX0.0, %IW0
   * - Comparisons: %IX0.0 == TRUE, %IW0 > 100, etc.
   * - In simulation mode, always returns true
   */
  private async evaluateGuard(guard: string): Promise<boolean> {
    console.log(`Evaluating guard: ${guard}`);
    
    // In simulation mode, always allow transitions
    if (this.mode === "simulation") {
      return true;
    }

    // In hardware mode, evaluate against real I/O
    if (!this.runtimeClient) {
      console.warn("Hardware mode but no runtime client available");
      return false;
    }

    try {
      // Parse guard expression
      // Format: ADDRESS [OPERATOR VALUE]
      // Examples: "%IX0.0", "%IX0.0 == TRUE", "%IW0 > 100"
      
      const trimmed = guard.trim();
      
      // Extract I/O address (IEC 61131-3 format: %[I|Q][X|W|D|L]...)
      const addressMatch = trimmed.match(/(%[IQM][XWDLB]\d+\.\d+|%[IQM][WDL]\d+)/i);
      if (!addressMatch) {
        console.warn(`No valid I/O address found in guard: ${guard}`);
        return false;
      }

      const address = addressMatch[0];
      
      // Read value from runtime
      const ioValue = await this.runtimeClient.readIo(address);
      console.log(`Guard I/O read: ${address} = ${ioValue}`);

      // If guard is just an address, treat as boolean
      if (trimmed === address) {
        return this.toBool(ioValue);
      }

      // Parse comparison operator and expected value
      const comparisonMatch = trimmed.match(/(%[IQM][XWDLB]\d+\.\d+|%[IQM][WDL]\d+)\s*(==|!=|>|>=|<|<=)\s*(.+)/i);
      if (!comparisonMatch) {
        console.warn(`Could not parse guard comparison: ${guard}`);
        return false;
      }

      const operator = comparisonMatch[2];
      const expectedValueStr = comparisonMatch[3].trim();
      
      // Convert expected value
      const expectedValue = this.parseValue(expectedValueStr);
      
      // Perform comparison
      return this.compareValues(ioValue, operator, expectedValue);
    } catch (error) {
      console.error(`Error evaluating guard: ${guard}`, error);
      return false;
    }
  }

  /**
   * Convert value to boolean
   */
  private toBool(value: any): boolean {
    if (typeof value === "boolean") {
      return value;
    }
    if (typeof value === "string") {
      return value.toLowerCase() === "true" || value === "1";
    }
    if (typeof value === "number") {
      return value !== 0;
    }
    return false;
  }

  /**
   * Parse string value to appropriate type
   */
  private parseValue(str: string): any {
    const upper = str.toUpperCase();
    if (upper === "TRUE") return true;
    if (upper === "FALSE") return false;
    if (/^-?\d+$/.test(str)) return parseInt(str, 10);
    if (/^-?\d+\.\d+$/.test(str)) return parseFloat(str);
    return str;
  }

  /**
   * Compare two values using operator
   */
  private compareValues(actual: any, operator: string, expected: any): boolean {
    switch (operator) {
      case "==":
        return actual == expected; // Loose equality for type coercion
      case "!=":
        return actual != expected;
      case ">":
        return Number(actual) > Number(expected);
      case ">=":
        return Number(actual) >= Number(expected);
      case "<":
        return Number(actual) < Number(expected);
      case "<=":
        return Number(actual) <= Number(expected);
      default:
        console.warn(`Unknown operator: ${operator}`);
        return false;
    }
  }

  /**
   * Check if machine is in a final state
   */
  isFinal(): boolean {
    const state = this.config.states[this.currentState];
    return state?.type === "final";
  }

  /**
   * Get current state name
   */
  getCurrentState(): string {
    return this.currentState;
  }
}
