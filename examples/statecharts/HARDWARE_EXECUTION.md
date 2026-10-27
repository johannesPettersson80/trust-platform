# StateChart Hardware Execution Guide

This guide explains how a StateChart drives **real I/O** on EtherCAT or other hardware.

A statechart runs as Structured Text, like every visual program
(`docs/specs/17-visual-editors-runtime-unification.md`). The StateChart Editor
has no execution mode of its own: its Simulation and Hardware modes, its
TypeScript engine and its runtime client were removed on 26 October 2026
(`docs/specs/25-vscode-product-contract.md` §11.4.1).

## How a statechart reaches the hardware

1. **Save the chart.** Beside `<name>.statechart.json` truST writes:
   - the companion `<name>.st`: `FUNCTION_BLOCK FB_<name>_STATECHART`, which
     holds the active state, takes each event of an untimed transition as a
     `BOOL` input `EV_<EVENT>`, and runs the mapped actions of each transition
     it takes;
   - the runtime entry `<name>.visual.runtime.st`: a `PROGRAM` that calls the
     instance `fb_<name>` and a `CONFIGURATION` that runs it every 20 ms and
     declares every direct address the chart writes as a global `AT` that
     address (`%QX0.0` becomes `ld_io_QX0_0 AT %QX0.0 : BOOL`).
2. **Map the addresses to the hardware** in the project's `io.toml`.
   `examples/hardware_8do/io.toml` maps `%QX0.0`–`%QX0.7` to an EK1100 +
   EL2008 (EtherCAT) and shows a Raspberry Pi GPIO alternative.
3. **Start it** from the truST sidebar, like any Structured Text program. The
   runtime writes the outputs the companion sets to the hardware every cycle.
4. **Raise events** by setting `fb_<name>.EV_<EVENT>` to `TRUE` in the
   debugger's Variables view (Set Value) and back to `FALSE`: every scan in
   which an event input is `TRUE` takes that event's transition from the
   active state. A transition with `after` fires on its own timer.
5. **Watch and force** the outputs in Live Values.

## Action mappings

Your `.statechart.json` names actions in `entry`, `exit` and transition
`actions`; `actionMappings` says what each one does:

```json
{
  "id": "ethercat_demo",
  "initial": "Init",
  "states": {
    "Init": {
      "on": { "START": "LED_0" }
    },
    "LED_0": {
      "entry": ["turnOn_DO0"],
      "exit": ["turnOff_DO0"],
      "on": { "TICK": "LED_1" }
    },
    "LED_1": {
      "entry": ["turnOn_DO1"],
      "exit": ["turnOff_DO1"],
      "on": { "TICK": "LED_0" }
    }
  },
  "actionMappings": {
    "turnOn_DO0": { "action": "WRITE_OUTPUT", "address": "%QX0.0", "value": true },
    "turnOff_DO0": { "action": "WRITE_OUTPUT", "address": "%QX0.0", "value": false },
    "turnOn_DO1": { "action": "WRITE_OUTPUT", "address": "%QX0.1", "value": true },
    "turnOff_DO1": { "action": "WRITE_OUTPUT", "address": "%QX0.1", "value": false }
  }
}
```

| Action type | Companion |
|---|---|
| `WRITE_OUTPUT` | writes `value` to `address` |
| `WRITE_VARIABLE` | writes `value` to `variable` |
| `SET_MULTIPLE` | writes each `targets` entry's `value` to its `address` |
| `LOG` | a comment with `message`; no effect when the program runs |

A taken transition runs the exit actions of its state, its own actions, then
the entry actions of its target. The editor's action-mapping panel lists the
actions that states and transitions name without a mapping, and the mappings
nothing names.

## Troubleshooting

### ❌ No `<name>.st` or `<name>.visual.runtime.st` beside the chart

Save the chart, or run Sync Structured Text. When the companion cannot be
made, the save shows why (for example an address the runtime refuses, such as
`%IX0.9`).

### ❌ The chart runs but the hardware doesn't respond

1. Check your `io.toml` driver configuration
2. Verify hardware connections (EtherCAT bus, GPIO pins)
3. Check that the addresses in `actionMappings` match your hardware layout
4. View trust-runtime logs for I/O errors

### ⚠️ An action does nothing

An action without a mapping becomes a comment in the companion. Map every
action that states and transitions name; the action-mapping panel lists the
missing ones.

---

## 📊 EtherCAT Example

See the complete working examples:
- [ethercat-snake-simple.statechart.json](ethercat-snake-simple.statechart.json) - 3 LEDs, best for learning
- [ethercat-snake-bidirectional.statechart.json](ethercat-snake-bidirectional.statechart.json) - Full 8 LEDs Knight Rider
- [ETHERCAT_SNAKE_README.md](ETHERCAT_SNAKE_README.md) - Full hardware setup guide

---

## 🎓 Best Practices

### 1. Use Descriptive Action Names
```json
"entry": ["turnOn_ConveyorMotor", "enableSafetyLight"]
```
Better than: `"entry": ["action1", "action2"]`

### 2. Group Related Actions
```json
"entry": ["stopAllMotors", "resetCounters", "enableEstop"]
```

### 3. Test Edge Cases
- Several events in one scan
- Unexpected events in wrong states
- Guard conditions blocking transitions

---

## 📚 Related Documentation

- [StateChart Examples README](README.md) - All available examples
- [EtherCAT Snake Guide](ETHERCAT_SNAKE_README.md) - Hardware setup details
- [IEC 61131-3 Addressing](https://en.wikipedia.org/wiki/IEC_61131-3#Addressing) - Address format reference
